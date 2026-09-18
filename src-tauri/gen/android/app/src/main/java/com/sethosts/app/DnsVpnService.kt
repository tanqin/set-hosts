package com.sethosts.app

import android.content.Intent
import android.net.VpnService
import android.os.ParcelFileDescriptor
import android.system.OsConstants
import java.io.FileInputStream
import java.io.FileOutputStream
import java.lang.ref.WeakReference
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.util.Collections
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * DNS 隧道：把系统 DNS 查询接进来，转交给应用内的本地 DNS 服务器（127.0.0.1:<port>）。
 *
 * 为什么要在隧道里自己造包：
 * - 普通应用无法绑定 53 端口，系统解析器也不会去访问 127.0.0.1:5353；
 * - 所以隧道里只声明 `10.111.222.0/30` 一条路由，并把隧道内 DNS 服务器设为 `10.111.222.2`，
 *   系统解析器发往该地址的 UDP 53 报文会被路由进隧道，本服务读出后转交给本地 DNS 服务器，
 *   再把应答按 IPv4/UDP 重新封装写回隧道。
 *
 * 其余流量不经过隧道，对系统网络无影响。
 */
class DnsVpnService : VpnService() {

  companion object {
    /** 本地 DNS 服务器端口，由 Rust 侧通过 Intent 传入 */
    const val EXTRA_PORT = "port"

    /** 隧道内网段：10.111.222.1 是本机地址，10.111.222.2 是隧道内「DNS 服务器」地址 */
    private const val TUN_ADDRESS = "10.111.222.1"
    private const val TUN_NETWORK = "10.111.222.0"
    private const val DNS_ADDRESS = "10.111.222.2"
    private const val PREFIX_LENGTH = 30
    private const val MTU = 1500

    private const val DNS_PORT = 53
    private const val UDP_PROTOCOL = 17
    private const val IPV4_HEADER_LENGTH = 20
    private const val UDP_HEADER_LENGTH = 8

    private const val LOOPBACK = "127.0.0.1"
    /**
     * 等本地 DNS 服务器应答的上限。
     *
     * 这条隧道接管的是「整台设备」的 DNS，一旦本地服务器无应答，超时就会被所有应用一起承受，
     * 所以不能设得太长——2 秒足够本机环回应答，又能让解析器尽快失败重试，不至于把整机网络拖死。
     */
    private const val FORWARD_TIMEOUT_MS = 2000
    private const val MAX_DNS_PAYLOAD = 4096
    private const val DEFAULT_DNS_PORT = 5353
    /** 工作线程数：系统解析器是并发查询的，线程太少会让查询排队 */
    private const val WORKER_COUNT = 8
    /** 隧道建立后多久还没收到任何查询就记一条警告（排查「系统没把 DNS 交给我们」） */
    private const val NO_QUERY_WATCHDOG_MS = 15_000L
    /** 等待旧读线程退出的最长时间：超时说明它卡在已关闭的描述符上 */
    private const val TEARDOWN_JOIN_TIMEOUT_MS = 1000L

    private val DNS_ADDRESS_BYTES = byteArrayOf(10, 111, 0xDE.toByte(), 2)

    @Volatile
    private var established = false

    /** 当前运行中的 service 实例弱引用，用于在 stopService 不可靠时直接让它自停 */
    private var instance: WeakReference<DnsVpnService>? = null

    /** 隧道是否已建立（Rust 侧通过 JNI 查询） */
    fun isRunning(): Boolean = established

    /** 直接要求当前运行中的 service 实例停止自己。
     *
     * 某些系统对 `stopService` 的处理有延迟，甚至根本不销毁 Service，导致状态栏 VPN
     * 图标一直挂着。通过持有实例引用直接 teardown + stopSelf，作为 stopService 的保险。
     */
    fun stopInstance() {
      instance?.get()?.let { service ->
        runCatching {
          service.teardown()
          service.stopSelf()
        }
      }
    }
  }

  private var tun: ParcelFileDescriptor? = null
  private var input: FileInputStream? = null
  private var output: FileOutputStream? = null
  private var reader: Thread? = null
  private var workers: ExecutorService? = null
  @Volatile
  private var session = 0

  @Volatile
  private var dnsPort = DEFAULT_DNS_PORT

  /** 是否已经打过「首个查询成功转发」的日志（用于真机排查，只打一次避免刷屏） */
  @Volatile
  private var loggedFirstForward = false

  /** 各工作线程复用的转发通道；teardown 时要统一关闭 */
  private val forwarders: MutableSet<Forwarder> = Collections.synchronizedSet(HashSet())

  /** 每个工作线程各自持有一个已连接到本地 DNS 服务器的 UDP socket */
  private val localForwarder = ThreadLocal<Forwarder>()

  override fun onCreate() {
    super.onCreate()
    instance = WeakReference(this)
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    DnsVpnDiagnostics.init(this)
    val requested = intent?.getIntExtra(EXTRA_PORT, dnsPort) ?: dnsPort
    if (established) {
      // 隧道已建立：只热更新转发端口，不重建
      dnsPort = requested
      return START_NOT_STICKY
    }
    teardown()
    dnsPort = requested
    if (!establish()) {
      stopSelf()
    }
    return START_NOT_STICKY
  }

  override fun onRevoke() {
    DnsVpnDiagnostics.warn("系统撤销了 VPN 授权（或被其它 VPN 应用抢占），关闭隧道")
    super.onRevoke()
    teardown()
    stopSelf()
  }

  override fun onDestroy() {
    instance = null
    teardown()
    super.onDestroy()
  }

  /** 建立只承载 DNS 的隧道 */
  private fun establish(): Boolean {
    val builder = Builder()
      .setSession("Set Hosts")
      .setMtu(MTU)
      .addAddress(TUN_ADDRESS, PREFIX_LENGTH)
      .addDnsServer(DNS_ADDRESS)
      .addRoute(TUN_NETWORK, PREFIX_LENGTH)
      // 【关键】VpnService 的规则：某个地址族只要「没有任何地址/路由/DNS 条目」，该族的
      // 全部流量都会被系统阻断。本隧道只声明了 IPv4，如果不解除，IPv6 会被整族掐死：
      //  - 别的应用在有 IPv6 的网络下直接上不了网（这是「开了 VPN 影响其他应用」的根因）；
      //  - 浏览器按 RFC 8305 优先试 IPv6，拿到公网 AAAA 后连不通，就表现为
      //    「域名打不开、直接访问 IP 却正常」。
      // allowFamily 只解除阻断，分配流量仍按路由走：我们没加任何 IPv6 路由，
      // 所以 IPv6 会正常落到下层网络，不会进隧道。
      .allowFamily(OsConstants.AF_INET)
      .allowFamily(OsConstants.AF_INET6)
      .setBlocking(true)

    // 把本应用自身排除在隧道之外，保证我们的上游 DNS 查询走底层网络、绝不绕回自己
    var excluded = true
    runCatching { builder.addDisallowedApplication(packageName) }
      .onFailure {
        excluded = false
        DnsVpnDiagnostics.warn("把本应用排除出隧道失败（不影响隧道建立）：${it.message}")
      }

    val descriptor = try {
      builder.establish()
    } catch (e: Exception) {
      DnsVpnDiagnostics.establishment = "建立失败：$e"
      DnsVpnDiagnostics.warn("建立隧道失败: $e")
      null
    }
    if (descriptor == null) {
      if (DnsVpnDiagnostics.establishment == "尚未建立") {
        DnsVpnDiagnostics.establishment = "establish() 返回空：用户未授权 VPN，或系统拒绝了隧道"
      }
      DnsVpnDiagnostics.warn("VPN 未授权或隧道建立失败")
      return false
    }
    tun = descriptor
    input = FileInputStream(descriptor.fileDescriptor)
    output = FileOutputStream(descriptor.fileDescriptor)
    workers = Executors.newFixedThreadPool(WORKER_COUNT)
    session += 1
    val mySession = session
    loggedFirstForward = false
    established = true
    reader = Thread({ readLoop(mySession) }, "dns-vpn-reader").apply {
      isDaemon = true
      start()
    }
    DnsVpnDiagnostics.tunnelActive = true
    DnsVpnDiagnostics.dnsPort = dnsPort
    DnsVpnDiagnostics.establishment =
      "地址 $TUN_ADDRESS/$PREFIX_LENGTH、隧道内 DNS $DNS_ADDRESS、路由 $TUN_NETWORK/$PREFIX_LENGTH、" +
        "allowFamily(IPv4+IPv6)、本应用已排除=$excluded；转发到 127.0.0.1:$dnsPort，" +
        "工作线程 $WORKER_COUNT，超时 ${FORWARD_TIMEOUT_MS}ms"
    DnsVpnDiagnostics.log("DNS 隧道已建立：${DnsVpnDiagnostics.establishment}")

    // 建立后一段时间仍收不到任何查询 → 说明系统没把 DNS 指向本隧道，
    // 这是「域名映射完全没生效」最常见的一种，单独记一笔免得用户对着空日志猜
    Thread({
      runCatching { Thread.sleep(NO_QUERY_WATCHDOG_MS) }
      if (established && mySession == session && DnsVpnDiagnostics.queryCount == 0) {
        DnsVpnDiagnostics.warn(
          "隧道建立 ${NO_QUERY_WATCHDOG_MS / 1000} 秒仍未收到任何 DNS 查询，系统可能没有把 DNS " +
            "指向本隧道：请检查是否开启了「私人 DNS / DoT」，或是否有其它 VPN 应用在抢占"
        )
      }
    }, "dns-vpn-watchdog").apply { isDaemon = true }.start()
    return true
  }

  private fun teardown() {
    if (established) {
      DnsVpnDiagnostics.log("DNS 隧道已关闭")
    }
    established = false
    DnsVpnDiagnostics.tunnelActive = false
    // 让读线程的会话编号失效，它即使还在跑也不会再处理报文
    session += 1
    val thread = reader
    reader = null
    workers?.shutdownNow()
    workers = null
    // 关闭各工作线程复用的转发通道，避免描述符泄漏
    synchronized(forwarders) {
      forwarders.forEach { runCatching { it.close() } }
      forwarders.clear()
    }
    runCatching { input?.close() }
    runCatching { output?.close() }
    runCatching { tun?.close() }
    input = null
    output = null
    tun = null
    // 等旧读线程退出后再重建，避免 fd 复用被抢走报文
    if (thread != null && thread !== Thread.currentThread()) {
      runCatching { thread.join(TEARDOWN_JOIN_TIMEOUT_MS) }
    }
  }

  /** 读线程：从隧道取出系统解析器的 DNS 查询 */
  private fun readLoop(mySession: Int) {
    val source = input ?: return
    val buffer = ByteArray(MTU)
    while (established && mySession == session) {
      val length = try {
        source.read(buffer)
      } catch (e: Exception) {
        if (established) {
          DnsVpnDiagnostics.warn("读取隧道报文失败: ${e.message}")
        }
        break
      }
      if (length <= 0) break
      val query = parseQuery(buffer, length) ?: continue
      val pool = workers ?: break
      if (pool.isShutdown) break
      pool.execute { answer(query, mySession) }
    }
    DnsVpnDiagnostics.log("隧道读线程退出")
  }

  /** 从隧道报文里取出 DNS 查询（只处理发往隧道内 DNS 服务器的 IPv4/UDP 报文） */
  private fun parseQuery(packet: ByteArray, length: Int): Query? {
    if (length < IPV4_HEADER_LENGTH + UDP_HEADER_LENGTH + 1) return null
    if ((packet[0].toInt() and 0xF0) != 0x40) return null
    val headerLength = (packet[0].toInt() and 0x0F) * 4
    if (headerLength < IPV4_HEADER_LENGTH || length < headerLength + UDP_HEADER_LENGTH) return null
    if (packet[9].toInt() != UDP_PROTOCOL) return null

    val clientPort = readShort(packet, headerLength)
    val destinationPort = readShort(packet, headerLength + 2)
    if (destinationPort != DNS_PORT) return null

    val udpLength = readShort(packet, headerLength + 4)
    val available = length - headerLength - UDP_HEADER_LENGTH
    var payloadLength =
      if (udpLength >= UDP_HEADER_LENGTH) udpLength - UDP_HEADER_LENGTH else available
    if (payloadLength > available) payloadLength = available
    if (payloadLength <= 0) return null

    val start = headerLength + UDP_HEADER_LENGTH
    return Query(
      clientAddress = packet.copyOfRange(12, 16),
      clientPort = clientPort,
      payload = packet.copyOfRange(start, start + payloadLength)
    )
  }

  /** 转发给本地 DNS 服务器，并把应答写回隧道 */
  private fun answer(query: Query, mySession: Int) {
    if (mySession != session) return
    val response = forward(query.payload) ?: return
    val total = IPV4_HEADER_LENGTH + UDP_HEADER_LENGTH + response.size
    if (total > MTU) {
      DnsVpnDiagnostics.warn("DNS 应答超出隧道 MTU（$total 字节），已丢弃")
      return
    }
    val frame = buildResponse(query, response)
    val sink = output ?: return
    synchronized(sink) {
      runCatching { sink.write(frame) }
    }
  }

  /** 交给应用内的本地 DNS 服务器（127.0.0.1:<port>）解析 */
  private fun forward(payload: ByteArray): ByteArray? {
    return try {
      val socket = forwarder().socket(dnsPort)
      socket.send(DatagramPacket(payload, payload.size))
      val buffer = ByteArray(MAX_DNS_PAYLOAD)
      val packet = DatagramPacket(buffer, buffer.size)
      socket.receive(packet)
      val response = buffer.copyOf(packet.length)
      if (!loggedFirstForward) {
        loggedFirstForward = true
        DnsVpnDiagnostics.log("首个查询转发成功：本地 DNS 服务器 127.0.0.1:$dnsPort 应答 ${packet.length} 字节")
      }
      DnsVpnDiagnostics.queryCount += 1
      DnsVpnDiagnostics.log("查询 ${describeQuery(payload)} → ${describeResponse(response)}")
      response
    } catch (e: Exception) {
      // 这条日志是真机定位的关键：它说明系统解析器确实把查询送进来了，
      // 只是本地 DNS 服务器没应答（隧道本身是通的）
      DnsVpnDiagnostics.forwardFailCount += 1
      DnsVpnDiagnostics.warn(
        "本地 DNS 服务器 127.0.0.1:$dnsPort 无应答 [${describeQuery(payload)}] ${e.message}"
      )
      null
    }
  }

  /** 取当前线程的转发通道（每个工作线程复用一个已连接的 UDP socket，避免每次查询都建 socket） */
  private fun forwarder(): Forwarder {
    localForwarder.get()?.let { return it }
    val fresh = Forwarder()
    forwarders.add(fresh)
    localForwarder.set(fresh)
    return fresh
  }

  /** 从 DNS 报文里取出问题名与查询类型（仅用于日志） */
  private fun describeQuery(payload: ByteArray): String {
    if (payload.size < 13) return "<报文过短>"
    val name = StringBuilder()
    var index = 12
    while (index < payload.size) {
      val length = payload[index].toInt() and 0xFF
      if (length == 0) {
        index += 1
        break
      }
      if (length > 63 || index + 1 + length > payload.size) return "<域名非法>"
      if (name.isNotEmpty()) name.append('.')
      name.append(String(payload, index + 1, length, Charsets.US_ASCII))
      index += 1 + length
    }
    val type = if (index + 1 < payload.size) readShort(payload, index) else -1
    return "$name(类型 $type)"
  }

  /** 应答摘要：RCODE 与答案条数（仅用于日志） */
  private fun describeResponse(payload: ByteArray): String {
    if (payload.size < 12) return "应答过短"
    val rcode = payload[3].toInt() and 0x0F
    return "rcode=$rcode 答案=${readShort(payload, 6)} 共 ${payload.size} 字节"
  }

  /** 单个工作线程的转发通道：连接到本地 DNS 服务器，复用同一个 socket */
  private class Forwarder {
    private var port = -1
    private var socket: DatagramSocket? = null

    @Synchronized
    fun socket(target: Int): DatagramSocket {
      val existing = socket
      if (existing != null && port == target && !existing.isClosed) return existing
      runCatching { existing?.close() }
      val fresh = DatagramSocket()
      fresh.soTimeout = FORWARD_TIMEOUT_MS
      fresh.connect(InetAddress.getByName(LOOPBACK), target)
      port = target
      socket = fresh
      return fresh
    }

    @Synchronized
    fun close() {
      runCatching { socket?.close() }
      socket = null
      port = -1
    }
  }

  /** 把 DNS 应答封装成「隧道内 DNS 服务器 → 客户端」的 IPv4/UDP 报文 */
  private fun buildResponse(query: Query, payload: ByteArray): ByteArray {
    val udpLength = UDP_HEADER_LENGTH + payload.size
    val total = IPV4_HEADER_LENGTH + udpLength
    val frame = ByteArray(total)

    // IPv4 头
    frame[0] = 0x45
    writeShort(frame, 2, total)
    writeShort(frame, 6, 0x4000) // 不分片
    frame[8] = 64 // TTL
    frame[9] = UDP_PROTOCOL.toByte()
    DNS_ADDRESS_BYTES.copyInto(frame, 12)
    query.clientAddress.copyInto(frame, 16)
    writeShort(frame, 10, checksum(frame, 0, IPV4_HEADER_LENGTH))

    // UDP 头
    writeShort(frame, IPV4_HEADER_LENGTH, DNS_PORT)
    writeShort(frame, IPV4_HEADER_LENGTH + 2, query.clientPort)
    writeShort(frame, IPV4_HEADER_LENGTH + 4, udpLength)
    payload.copyInto(frame, IPV4_HEADER_LENGTH + UDP_HEADER_LENGTH)

    // UDP 校验和（含伪首部）
    val pseudo = ByteArray(12 + udpLength)
    DNS_ADDRESS_BYTES.copyInto(pseudo, 0)
    query.clientAddress.copyInto(pseudo, 4)
    pseudo[9] = UDP_PROTOCOL.toByte()
    writeShort(pseudo, 10, udpLength)
    frame.copyInto(pseudo, 12, IPV4_HEADER_LENGTH, total)
    var udpChecksum = checksum(pseudo, 0, pseudo.size)
    if (udpChecksum == 0) {
      udpChecksum = 0xFFFF
    }
    writeShort(frame, IPV4_HEADER_LENGTH + 6, udpChecksum)

    return frame
  }

  private class Query(
    val clientAddress: ByteArray,
    val clientPort: Int,
    val payload: ByteArray
  )

  private fun readShort(data: ByteArray, offset: Int): Int =
    ((data[offset].toInt() and 0xFF) shl 8) or (data[offset + 1].toInt() and 0xFF)

  private fun writeShort(data: ByteArray, offset: Int, value: Int) {
    data[offset] = ((value shr 8) and 0xFF).toByte()
    data[offset + 1] = (value and 0xFF).toByte()
  }

  /** 标准的 16 位反码校验和 */
  private fun checksum(data: ByteArray, offset: Int, length: Int): Int {
    var sum = 0
    var index = offset
    val end = offset + length
    while (index + 1 < end) {
      sum += readShort(data, index)
      index += 2
    }
    if (index < end) {
      sum += (data[index].toInt() and 0xFF) shl 8
    }
    while ((sum shr 16) != 0) {
      sum = (sum and 0xFFFF) + (sum shr 16)
    }
    return sum.inv() and 0xFFFF
  }
}

package com.sethosts.app

import android.content.Context
import android.util.Log
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * 隧道诊断日志：内存环形缓冲 + 节流落盘。
 *
 * VPN 隧道跑在 Service 里，用户界面拿不到它的 logcat，真机上也不一定有条件用 adb，
 * 所以把关键事件同时留在内存和私有目录文件里，由应用内「诊断日志」页读出来展示 / 复制。
 *
 * 之所以要记这么细：真机上「域名打不开但直接连 IP 正常」这类问题，必须区分是
 * ① 查询根本没进隧道 ② 进了隧道但本地 DNS 服务器没应答 ③ 应答了但答案不对。
 * 这三者只有把每条查询和应答都记下来才分得清。
 */
object DnsVpnDiagnostics {
    private const val TAG = "DnsVpn"

    /** 落盘文件（应用私有目录，卸载随应用一起清除） */
    private const val FILE_NAME = "dns-vpn.log"

    /** 单个日志文件上限，超过后整体重写为内存里最近的那些行 */
    private const val MAX_FILE_BYTES = 192 * 1024

    /** 内存里保留的最近行数 */
    private const val MAX_LINES = 800

    /** 落盘节流间隔：DNS 查询很频繁，每条都落盘会白白耗电 */
    private const val FLUSH_INTERVAL_MS = 1000L

    private val lock = Any()
    private val recent = ArrayDeque<String>()
    private val pending = ArrayDeque<String>()
    private val timeFormat = SimpleDateFormat("HH:mm:ss.SSS", Locale.US)

    private var file: File? = null
    private var lastFlush = 0L

    // ---- 结构化状态：比日志本身更关键，且不随日志滚动丢失 ----

    /** 隧道是否已建立 */
    @Volatile
    var tunnelActive = false

    /** 隧道内转发目标的端口（127.0.0.1:<port>） */
    @Volatile
    var dnsPort = 0

    /** 建立隧道时实际生效的参数摘要，或失败原因 */
    @Volatile
    var establishment = "尚未建立"

    /** 累计接到的 DNS 查询数 */
    @Volatile
    var queryCount = 0

    /** 累计「本地 DNS 服务器无应答」次数 */
    @Volatile
    var forwardFailCount = 0

    /** 用户在系统 VPN 授权弹窗里是否拒绝过 */
    @Volatile
    var consentDeclined = false

    fun init(context: Context) {
        synchronized(lock) {
            if (file == null) {
                file = File(context.applicationContext.filesDir, FILE_NAME)
            }
        }
    }

    fun log(message: String) {
        Log.i(TAG, message)
        append(message)
    }

    fun warn(message: String) {
        Log.w(TAG, message)
        append("WARN $message")
    }

    private fun append(message: String) {
        synchronized(lock) {
            val line = "${timeFormat.format(Date())} $message"
            recent.addLast(line)
            while (recent.size > MAX_LINES) {
                recent.removeFirst()
            }
            pending.addLast(line)
            if (System.currentTimeMillis() - lastFlush >= FLUSH_INTERVAL_MS) {
                flushLocked()
            }
        }
    }

    /** 把尚未落盘的行写进文件（须持有 [lock]） */
    private fun flushLocked() {
        if (pending.isEmpty()) return
        lastFlush = System.currentTimeMillis()
        val target = file ?: return
        runCatching {
            if (target.length() > MAX_FILE_BYTES) {
                // 超限时整体重写：内存里的 recent 已包含 pending 的内容，不会丢行
                target.writeText(recent.joinToString("\n", postfix = "\n"))
            } else {
                target.appendText(pending.joinToString("\n", postfix = "\n"))
            }
            pending.clear()
        }
    }

    /** 生成给用户看 / 复制走的完整报告 */
    fun dump(): String {
        synchronized(lock) {
            flushLocked()
            val onDisk = runCatching {
                file?.takeIf { it.exists() }?.readText().orEmpty()
            }.getOrDefault("")
            val body = onDisk.ifBlank { recent.joinToString("\n") }

            return buildString {
                appendLine("---- VPN 隧道（原生） ----")
                appendLine("隧道已建立: $tunnelActive")
                appendLine("转发目标: 127.0.0.1:$dnsPort")
                appendLine("建立参数: $establishment")
                appendLine("接到 DNS 查询: $queryCount 条（本地服务器无应答 $forwardFailCount 条）")
                appendLine("系统授权弹窗被拒绝过: $consentDeclined")
                appendLine("---- 隧道日志（最近 $MAX_LINES 行） ----")
                append(body)
                if (!body.endsWith("\n")) appendLine()
            }
        }
    }

    fun clear() {
        synchronized(lock) {
            recent.clear()
            pending.clear()
            queryCount = 0
            forwardFailCount = 0
            runCatching { file?.writeText("") }
        }
    }
}

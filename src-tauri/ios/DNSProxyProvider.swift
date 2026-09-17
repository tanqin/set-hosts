//  Set Hosts — iOS DNS 代理提供者（NEDNSProxyProvider）
//
//  作用：把系统 DNS 查询转发到应用内 Rust 本地 DNS 服务器（默认 127.0.0.1:5353），
//  使 hosts 映射在 iOS 上全系统生效，对应 Android 的 DnsVpnService。
//
//  集成方式见同目录 README.md：本文件需加入 Xcode 的
//  「Network Extension → DNS Proxy Provider」扩展 Target。
//
//  注意：Network Extension 是系统级能力，必须在付费开发者账号下为扩展 Target
//  申请 com.apple.developer.networking.networkextension（dns-proxy）授权，
//  否则管理器保存配置时会被系统拒绝。

import Network
import NetworkExtension

/// DNS 代理提供者：把拦截到的 DNS 报文转发给本机 Rust DNS 服务器
final class DNSProxyProvider: NEDNSProxyProvider {
    /// providerConfiguration 中读取本地 DNS 服务器端口的键（由 DNSProxyController 写入）
    private static let portKey = "dnsPort"
    private static let defaultPort: UInt16 = 5353

    private var dnsPort: UInt16 = DNSProxyProvider.defaultPort
    private var relay: DNSRelay?

    override func startProxy(
        options: [String: Any]? = nil,
        completionHandler: @escaping (Error?) -> Void
    ) {
        if let port = (protocolConfiguration as? NEDNSProxyProviderProtocol)?
            .providerConfiguration?[DNSProxyProvider.portKey] as? Int,
           port > 0, port <= Int(UInt16.max)
        {
            dnsPort = UInt16(port)
        }
        relay = DNSRelay(port: dnsPort)
        completionHandler(nil)
    }

    override func stopProxy(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        relay = nil
        completionHandler()
    }

    /// 系统每产生一条 DNS 流就回调这里；UDP 流即 DNS 查询
    override func handleNewFlow(_ flow: NEAppProxyFlow) -> Bool {
        guard let udpFlow = flow as? NEAppProxyUDPFlow else {
            // DNS 只会走 UDP / TCP 的 53 端口；其它流量一律不接管（本应用不做流量代理）
            return false
        }
        forward(udpFlow)
        return true
    }

    private func forward(_ flow: NEAppProxyUDPFlow) {
        flow.open(withLocalEndpoint: nil) { [weak self] error in
            guard let self = self else { return }
            if error != nil {
                flow.closeReadAndWrite()
                return
            }
            self.readNext(flow)
        }
    }

    private func readNext(_ flow: NEAppProxyUDPFlow) {
        flow.readDatagrams { [weak self] datagrams, endpoints, error in
            guard let self = self else { return }
            if error != nil || datagrams == nil {
                flow.closeReadAndWrite()
                return
            }
            guard let datagrams = datagrams, let endpoints = endpoints, !datagrams.isEmpty else {
                flow.closeReadAndWrite()
                return
            }
            for (index, datagram) in datagrams.enumerated() {
                let endpoint = endpoints[index]
                self.relay?.query(datagram) { [weak flow] reply in
                    guard let flow = flow, let reply = reply else { return }
                    flow.writeDatagrams([reply], sentBy: [endpoint]) { writeError in
                        if writeError != nil {
                            flow.closeReadAndWrite()
                        }
                    }
                }
            }
            self.readNext(flow)
        }
    }
}

/// 把单条 DNS 查询转发到本机 DNS 服务器并取回应答
///
/// DNS 查询是「一问一答」的短事务，这里为每条查询开一条 UDP 连接，
/// 收到第一个应答即关闭，避免长期占用扩展进程的资源。
private final class DNSRelay {
    private let port: UInt16
    private let queue = DispatchQueue(label: "com.sethosts.dnsproxy.relay")

    init(port: UInt16) {
        self.port = port
    }

    func query(_ data: Data, completion: @escaping (Data?) -> Void) {
        guard let port = NWEndpoint.Port(rawValue: port) else {
            completion(nil)
            return
        }
        let connection = NWConnection(
            host: .ipv4(.loopback),
            port: port,
            using: .udp
        )
        connection.stateUpdateHandler = { [weak connection] state in
            switch state {
            case .ready:
                connection?.send(content: data, completion: .contentProcessed { error in
                    if error != nil {
                        connection?.cancel()
                        completion(nil)
                        return
                    }
                    connection?.receiveMessage { reply, _, _, _, error in
                        connection?.cancel()
                        if error != nil || reply == nil {
                            completion(nil)
                        } else {
                            completion(reply)
                        }
                    }
                })
            case .failed, .cancelled:
                completion(nil)
            default:
                break
            }
        }
        connection.start(queue: queue)
    }
}

//  Set Hosts — iOS DNS 代理控制端（宿主 App 侧）
//
//  对应 Android 的 DnsVpn.kt：负责启停系统 DNS 代理、查询当前是否已接管。
//  真正的报文转发在扩展进程里的 DNSProxyProvider 完成。

import NetworkExtension

enum DNSProxyController {
    /// 扩展 Target 的 Bundle Identifier，需与 Xcode 中的 Product Bundle Identifier 一致
    static let providerBundleIdentifier = "com.sethosts.app.DNSProxyExtension"

    /// 启动 DNS 代理，把系统 DNS 交给本机 `127.0.0.1:<port>`
    static func start(port: UInt16) async throws {
        let manager = NEDNSProxyManager.shared()
        try await manager.loadFromPreferences()

        let proto = NEDNSProxyProviderProtocol()
        proto.providerConfiguration = ["dnsPort": Int(port)]
        proto.providerBundleIdentifier = providerBundleIdentifier

        manager.localizedDescription = "Set Hosts DNS Proxy"
        manager.providerProtocol = proto
        manager.isEnabled = true

        try await manager.saveToPreferences()
        // 保存后必须重新 load，否则 isEnabled 读到的还是旧值
        try await manager.loadFromPreferences()

        if !manager.isEnabled {
            throw DNSProxyError.notAuthorized
        }
    }

    /// 关闭 DNS 代理
    static func stop() async {
        let manager = NEDNSProxyManager.shared()
        do {
            try await manager.loadFromPreferences()
            manager.isEnabled = false
            try await manager.saveToPreferences()
        } catch {
            // 关闭失败无需中断退出流程
        }
    }

    /// 系统 DNS 是否已由本应用接管（对应 Android 的 is_vpn_active）
    static func isActive() async -> Bool {
        let manager = NEDNSProxyManager.shared()
        do {
            try await manager.loadFromPreferences()
        } catch {
            return false
        }
        return manager.isEnabled
    }
}

enum DNSProxyError: LocalizedError {
    case notAuthorized

    var errorDescription: String? {
        switch self {
        case .notAuthorized:
            return "系统未授权 DNS 代理：需要付费开发者账号的 Network Extension 授权（dns-proxy）"
        }
    }
}

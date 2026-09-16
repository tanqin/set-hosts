//! 移动端原生桥接：VPN 隧道把系统 DNS 指向应用内的本地 DNS 服务器
//!
//! 本地 DNS 服务器已由 [`crate::dns_proxy`] 实现；移动端要让映射真正生效，
//! 还需把系统 DNS 流量接管到该服务器（Android `VpnService` / iOS `NEPacketTunnelProvider`）：
//!
//! - Android：[`android::start_vpn`] 需在 `gen/android` 原生工程中实现 Kotlin 侧
//!   `VpnService.Builder.addDnsServer("127.0.0.1")`（回环地址不经隧道，可直接命中本地服务器）；
//! - iOS：[`ios::start_tunnel`] 需 Apple 付费开发者账号与 Network Extension entitlement。
//!
//! 未接入原生隧道时，[`is_tunnel_active`] 返回 false，前端会提示「未接管系统 DNS」。

#[allow(dead_code)]
pub mod android;
#[allow(dead_code)]
pub mod ios;

/// 启动 VPN 隧道，把系统 DNS 指向本地 DNS 服务器端口
pub fn start_tunnel(port: u16) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::start_vpn(port)
    }
    #[cfg(target_os = "ios")]
    {
        ios::start_tunnel(&format!("127.0.0.1:{}", port))
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = port;
        Err("当前平台无需 VPN 隧道".to_string())
    }
}

/// 停止 VPN 隧道
pub fn stop_tunnel() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::stop_vpn()
    }
    #[cfg(target_os = "ios")]
    {
        ios::stop_tunnel()
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Err("当前平台无需 VPN 隧道".to_string())
    }
}

/// VPN 隧道是否已接管系统 DNS
pub fn is_tunnel_active() -> bool {
    false
}

//! Android VpnService 桥接
//!
//! 目标实现（需 `tauri android init` 生成的 Kotlin 工程）：
//! ```kotlin
//! val builder = Builder()
//!     .setSession("Set Hosts")
//!     .addAddress("10.111.222.1", 32)
//!     .addDnsServer("127.0.0.1")   // 回环地址不被隧道拦截，直接命中应用内 DNS 服务器
//!     .addRoute("0.0.0.0", 0)      // 仅需 DNS 生效时可收窄路由
//! ```
//! 应用侧以 `protect()` 保护本地 DNS 服务器的套接字，避免自身流量被回环进隧道。

/// 启动 VPN 隧道，把系统 DNS 指向本地 DNS 服务器端口
pub fn start_vpn(port: u16) -> Result<(), String> {
    Err(format!(
        "Android VPN 隧道尚未接入原生 VpnService（本地 DNS 服务器已监听 127.0.0.1:{}，执行 tauri android init 后接入即可生效）",
        port
    ))
}

/// 停止 VPN 隧道
pub fn stop_vpn() -> Result<(), String> {
    Err("Android VPN 隧道尚未接入原生 VpnService".to_string())
}

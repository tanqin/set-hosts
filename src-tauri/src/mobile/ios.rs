//! iOS NetworkExtension 桥接
//!
//! 需要 Apple 付费开发者账号 + Network Extension entitlement；
//! 隧道内把 DNS 设置为 `127.0.0.1:<port>`（回环地址不经隧道，直接命中应用内 DNS 服务器），
//! 或使用 NEDNSProxyProvider 直接接管 DNS 查询。

/// 启动隧道，把系统 DNS 指向本地 DNS 服务器
pub fn start_tunnel(listen_addr: &str) -> Result<(), String> {
    Err(format!(
        "iOS Network Extension 尚未接入原生隧道（本地 DNS 服务器已监听 {}，需付费开发者账号 + entitlement 后才能生效）",
        listen_addr
    ))
}

/// 停止隧道
pub fn stop_tunnel() -> Result<(), String> {
    Err("iOS Network Extension 尚未接入原生隧道".to_string())
}

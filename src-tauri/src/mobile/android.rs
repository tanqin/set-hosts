//! Android VpnService 桥接（Phase 2）

/// 启动 VPN 隧道，把系统 DNS 指向本地 DNS 端口
pub fn start_vpn(_listen_port: u16) -> Result<(), String> {
    Err("Android VPN 隧道将在 Phase 2 启用".to_string())
}

/// 停止 VPN 隧道
pub fn stop_vpn() -> Result<(), String> {
    Err("Android VPN 隧道将在 Phase 2 启用".to_string())
}

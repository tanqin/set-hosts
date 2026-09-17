//! iOS NetworkExtension 桥接
//!
//! 需要 Apple 付费开发者账号 + Network Extension entitlement；
//! 隧道内把系统 DNS 查询转发给本机 `127.0.0.1:<port>`（即 Rust 本地 DNS 服务器），
//! 采用 `NEDNSProxyProvider` 接管（不需要虚拟网卡，比 NEPacketTunnelProvider 更贴合本应用）。
//!
//! # 当前状态
//!
//! 原生实现源码已提供在 `src-tauri/ios/`（`DNSProxyProvider.swift` / `DNSProxyController.swift`），
//! 但**必须在 macOS + Xcode 中完成接入**：`tauri ios init` 生成工程、新建 DNS Proxy 扩展 Target、
//! 申请 `dns-proxy` entitlement，再用 `swift-rs` 把 Swift 控制端桥接到本文件的两个函数。
//! 完整步骤见 `src-tauri/ios/README.md`。
//!
//! 隧道未接入时这里返回 `Err`，上层 `sync_mobile_tunnel` 会据此回滚配置开关——
//! 界面显示「已启用」而映射实际未生效，比开关打不开更糟。

/// 启动隧道，把系统 DNS 指向本地 DNS 服务器
pub fn start_tunnel(listen_addr: &str) -> Result<(), String> {
    Err(format!(
        "iOS Network Extension 尚未接入原生隧道（本地 DNS 服务器已监听 {}，\
         需付费开发者账号 + entitlement 后才能生效；接入步骤见 src-tauri/ios/README.md）",
        listen_addr
    ))
}

/// 停止隧道
pub fn stop_tunnel() -> Result<(), String> {
    Err("iOS Network Extension 尚未接入原生隧道".to_string())
}

//! iOS NetworkExtension 桥接（Phase 2）
//!
//! 需要 Apple 付费开发者账号 + Network Extension entitlement。

pub fn start_tunnel(_listen_addr: &str) -> Result<(), String> {
    Err("iOS Network Extension 将在 Phase 2 启用（需 Apple 付费账号 + entitlement）".to_string())
}

pub fn stop_tunnel() -> Result<(), String> {
    Err("iOS Network Extension 将在 Phase 2 启用".to_string())
}

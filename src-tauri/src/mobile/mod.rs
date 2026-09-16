//! 移动端原生桥接（Phase 2 填充）
//!
//! Phase 1：提供命令骨架，调用时返回「未实现」。
//! Phase 2：Android 通过 Tauri plugin API 调 Kotlin VpnService；
//!          iOS 通过 plugin 调 Swift NEPacketTunnelProvider。

#[allow(dead_code)]
pub mod android;
#[allow(dead_code)]
pub mod ios;

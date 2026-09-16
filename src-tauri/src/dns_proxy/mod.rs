//! DNS 代理模块（移动端，feature gate "dns-proxy"）
//!
//! 桌面端默认不启用此模块，相关命令返回错误。
//! 移动端通过 VPN 隧道 + 本地 DNS 服务器让 hosts 映射生效，无需 root/越狱。

pub mod server;
pub mod resolver;

use std::sync::Arc;
use tokio::sync::RwLock;

/// DNS 代理全局状态
pub struct ProxyState {
    /// 域名 → IP 映射（由当前激活 profile 渲染）
    pub mappings: RwLock<std::collections::HashMap<String, std::net::IpAddr>>,
    /// 运行中的 DNS server 句柄
    pub server: RwLock<Option<server::DnsServerHandle>>,
    /// 命中计数
    pub hit_count: std::sync::atomic::AtomicU64,
    /// 转发计数
    pub forward_count: std::sync::atomic::AtomicU64,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self {
            mappings: RwLock::new(std::collections::HashMap::new()),
            server: RwLock::new(None),
            hit_count: std::sync::atomic::AtomicU64::new(0),
            forward_count: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl ProxyState {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

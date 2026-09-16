//! DNS 代理：内置本地 DNS 服务器，让 hosts 映射无需写入系统 hosts 即可生效
//!
//! 工作方式：
//! 1. 把所有「启用 profile」的条目编译成 `域名 → IP` 映射表（[`mappings_from_config`]）；
//! 2. 本地 DNS 服务器（[`server`]）收到查询时，命中映射直接应答，未命中转发上游（[`resolver`]）；
//! 3. 移动端（Android/iOS）由 VPN 隧道把系统 DNS 指向本服务器（见 [`crate::mobile`]），
//!    桌面端也可手动启动，用于验证映射效果：
//!    `nslookup -port=5353 example.com 127.0.0.1`

pub mod resolver;
pub mod server;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;

use crate::models::{Config, ProxyStatus};

/// 默认监听端口：非特权端口，移动端与桌面端普通权限即可绑定
pub const DEFAULT_PORT: u16 = 5353;

/// 域名（小写、无末尾点）→ IP 映射表
pub type Mappings = HashMap<String, IpAddr>;

/// DNS 服务器运行统计
#[derive(Default)]
pub struct Counters {
    /// 命中映射并直接应答的查询数
    pub hit: AtomicU64,
    /// 转发到上游的查询数
    pub forward: AtomicU64,
}

/// DNS 代理全局状态（挂在 Tauri 全局状态上，跨命令共享）
pub struct DnsProxy {
    /// 域名 → IP 映射（服务器与状态共享，支持热更新，无需重启服务器）
    mappings: Arc<RwLock<Mappings>>,
    /// 上游 DNS 服务器列表
    upstream: Arc<RwLock<Vec<SocketAddr>>>,
    /// 运行统计
    counters: Arc<Counters>,
    /// 运行中的服务器句柄
    server: Mutex<Option<server::ServerHandle>>,
    /// 监听地址（同步读取，供状态查询）
    listen_addr: RwLock<Option<SocketAddr>>,
    /// 是否正在运行
    running: AtomicBool,
}

impl Default for DnsProxy {
    fn default() -> Self {
        Self {
            mappings: Arc::new(RwLock::new(Mappings::new())),
            upstream: Arc::new(RwLock::new(Vec::new())),
            counters: Arc::new(Counters::default()),
            server: Mutex::new(None),
            listen_addr: RwLock::new(None),
            running: AtomicBool::new(false),
        }
    }
}

impl DnsProxy {
    /// 创建共享实例
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 全量替换映射表，返回新的映射条数
    pub fn set_mappings(&self, mappings: Mappings) -> usize {
        let count = mappings.len();
        *self.mappings.write().unwrap_or_else(|e| e.into_inner()) = mappings;
        count
    }

    /// 当前映射条数
    pub fn mapping_count(&self) -> usize {
        self.mappings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 监听地址（未运行时为 None）
    pub fn listen_addr(&self) -> Option<SocketAddr> {
        *self.listen_addr.read().unwrap_or_else(|e| e.into_inner())
    }

    /// 启动 DNS 服务器（重复调用会先停止旧实例再按新参数启动）
    pub async fn start(
        &self,
        bind_addr: SocketAddr,
        upstream: Vec<SocketAddr>,
    ) -> Result<SocketAddr, String> {
        if upstream.is_empty() {
            return Err("未确定上游 DNS 服务器".to_string());
        }
        // 幂等：先停掉旧实例，避免端口占用
        self.stop().await?;

        *self.upstream.write().unwrap_or_else(|e| e.into_inner()) = upstream;

        let handle = server::start(
            bind_addr,
            self.mappings.clone(),
            self.upstream.clone(),
            self.counters.clone(),
        )
        .await?;

        let addr = handle.listen_addr;
        *self.listen_addr.write().unwrap_or_else(|e| e.into_inner()) = Some(addr);
        *self.server.lock().await = Some(handle);
        self.running.store(true, Ordering::Relaxed);
        Ok(addr)
    }

    /// 停止 DNS 服务器（未运行时为空操作）
    pub async fn stop(&self) -> Result<(), String> {
        if let Some(mut handle) = self.server.lock().await.take() {
            handle.stop();
        }
        self.running.store(false, Ordering::Relaxed);
        *self.listen_addr.write().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }

    /// 运行状态快照（供前端展示）
    pub fn status(&self) -> ProxyStatus {
        ProxyStatus {
            running: self.is_running(),
            listen_addr: self.listen_addr().map(|a| a.to_string()),
            hit_count: self.counters.hit.load(Ordering::Relaxed),
            forward_count: self.counters.forward.load(Ordering::Relaxed),
            mapping_count: self.mapping_count(),
            upstream: self
                .upstream
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .map(|a| a.to_string())
                .collect(),
            tunnel_active: crate::mobile::is_tunnel_active(),
        }
    }
}

/// 把所有启用 profile 的条目编译为 `域名 → IP` 映射
///
/// - 仅取启用的 profile 与启用中的条目；
/// - 同一域名在多个 profile 中重复时，以先出现的为准（列表顺序）；
/// - 忽略无法解析为 IP 的行（如纯注释条目）。
pub fn mappings_from_config(config: &Config) -> Mappings {
    let mut mappings = Mappings::new();

    for profile in config.profiles.iter().filter(|p| p.enabled) {
        for entry in crate::parser::parse_hosts_text(&profile.content) {
            if !entry.enabled || entry.ip.is_empty() {
                continue;
            }
            let Ok(ip) = entry.ip.parse::<IpAddr>() else {
                log::warn!("忽略无法解析的 hosts 地址：{}（{}）", entry.ip, profile.name);
                continue;
            };
            for domain in &entry.domains {
                let domain = normalize_domain(domain);
                if domain.is_empty() {
                    continue;
                }
                mappings.entry(domain).or_insert(ip);
            }
        }
    }

    mappings
}

/// 域名规范化：去掉首尾空白与末尾的点，统一小写
pub fn normalize_domain(domain: &str) -> String {
    domain.trim().trim_end_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Profile;

    fn profile(name: &str, content: &str, enabled: bool) -> Profile {
        Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            content: content.to_string(),
            enabled,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_applied_at: None,
            is_remote: false,
            url: None,
            last_fetch_at: None,
            auto_refresh_secs: 0,
        }
    }

    #[test]
    fn builds_mappings_only_from_enabled_profiles() {
        let config = Config {
            profiles: vec![
                profile("enabled", "10.0.0.1 dev.local api.local # 开发\n", true),
                profile("disabled", "10.0.0.2 off.local\n", false),
            ],
            active_profile_id: None,
            backups: vec![],
        };

        let mappings = mappings_from_config(&config);
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings.get("dev.local"), Some(&"10.0.0.1".parse().unwrap()));
        assert_eq!(mappings.get("api.local"), Some(&"10.0.0.1".parse().unwrap()));
        assert!(!mappings.contains_key("off.local"));
    }

    #[test]
    fn first_profile_wins_on_conflict_and_ignores_invalid_lines() {
        let config = Config {
            profiles: vec![
                profile("a", "10.0.0.1 dup.local\n# 注释\nnot-an-ip bad.local\n", true),
                profile("b", "10.0.0.2 dup.local\n", true),
            ],
            active_profile_id: None,
            backups: vec![],
        };

        let mappings = mappings_from_config(&config);
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings.get("dup.local"), Some(&"10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn supports_ipv6_and_normalizes_domain() {
        let config = Config {
            profiles: vec![profile("v6", "::1  LocalHost.\n", true)],
            active_profile_id: None,
            backups: vec![],
        };

        let mappings = mappings_from_config(&config);
        assert_eq!(mappings.get("localhost"), Some(&"::1".parse().unwrap()));
    }

    #[test]
    fn status_reflects_state() {
        let proxy = DnsProxy::default();
        proxy.set_mappings(mappings_from_config(&Config {
            profiles: vec![profile("p", "10.0.0.1 dev.local\n", true)],
            active_profile_id: None,
            backups: vec![],
        }));

        let status = proxy.status();
        assert!(!status.running);
        assert_eq!(status.mapping_count, 1);
        assert!(status.listen_addr.is_none());
        assert!(status.upstream.is_empty());
    }
}

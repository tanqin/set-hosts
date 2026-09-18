//! 数据模型：HostEntry / Profile / BackupRecord / ProxyStatus 等

use serde::{Deserialize, Serialize};

/// 一条 hosts 条目：一个 IP 对应一个或多个域名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    /// 唯一 ID（uuid v4）
    pub id: String,
    /// IP 地址（IPv4 或 IPv6）
    pub ip: String,
    /// 域名列表（一条可对应多个域名）
    pub domains: Vec<String>,
    /// 是否启用
    pub enabled: bool,
    /// 注释（可选）
    pub comment: Option<String>,
}

/// 一套 hosts 配置方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// 原始 hosts 文本内容（用户直接编辑；远程 profile 为最近一次拉取的内容）
    pub content: String,
    /// 是否启用（整个 profile 级别的开关）
    pub enabled: bool,
    pub created_at: String,
    pub last_applied_at: Option<String>,
    /// 是否为远程 hosts（内容来自 URL 拉取，不可手动编辑）
    #[serde(default)]
    pub is_remote: bool,
    /// 远程 hosts 的 URL（仅远程 profile 有值）
    #[serde(default)]
    pub url: Option<String>,
    /// 最近一次成功拉取时间
    #[serde(default)]
    pub last_fetch_at: Option<String>,
    /// 远程 hosts 自动刷新间隔（秒）；0 = 从不自动刷新
    #[serde(default)]
    pub auto_refresh_secs: u64,
}

/// hosts 文件备份记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,
    /// 备份时间（ISO 8601）
    pub timestamp: String,
    /// 备份时的 hosts 内容快照
    pub content: String,
    /// 来源路径（系统 hosts 路径）
    pub source_path: String,
}

/// DNS 代理运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    /// 本地 DNS 服务器监听地址（如 127.0.0.1:5353）
    pub listen_addr: Option<String>,
    /// 命中映射并直接应答的查询数
    pub hit_count: u64,
    /// 转发到上游的查询数
    pub forward_count: u64,
    /// 当前生效的映射条数（来自所有启用 profile）
    pub mapping_count: usize,
    /// 上游 DNS 服务器
    pub upstream: Vec<String>,
    /// 移动端 VPN 隧道是否已接管系统 DNS（桌面端恒为 false）
    pub tunnel_active: bool,
}

impl Default for ProxyStatus {
    fn default() -> Self {
        Self {
            running: false,
            listen_addr: None,
            hit_count: 0,
            forward_count: 0,
            mapping_count: 0,
            upstream: Vec::new(),
            tunnel_active: false,
        }
    }
}

/// 导入导出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Hosts,
    Json,
}

/// 导入结果摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub profile_count: usize,
    pub entry_count: usize,
}

/// 应用持久化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 全部配置方案
    pub profiles: Vec<Profile>,
    /// 当前激活的 profile id
    pub active_profile_id: Option<String>,
    /// 备份历史
    pub backups: Vec<BackupRecord>,
}

impl Default for Config {
    fn default() -> Self {
        // 首次启动时创建一个默认 profile（名称固定为英文 Default，与界面语言无关）
        let default_profile = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Default".to_string(),
            content: String::new(),
            enabled: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_applied_at: None,
            is_remote: false,
            url: None,
            last_fetch_at: None,
            auto_refresh_secs: 0,
        };
        let default_id = default_profile.id.clone();
        Self {
            profiles: vec![default_profile],
            active_profile_id: Some(default_id),
            backups: vec![],
        }
    }
}

impl Config {
    /// 按 id 查找 profile 的可变引用
    pub fn profile_mut(&mut self, id: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    /// 按 id 查找 profile
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
}

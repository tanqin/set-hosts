//! 导入导出：JSON / hosts 文本

use crate::models::{Config, ExportFormat, ImportSummary};

/// 导出：返回字符串内容
/// - Hosts：当前激活 profile 的原始 hosts 文本
/// - Json：整个 Config 序列化
pub fn export_config(config: &Config, format: &ExportFormat) -> Result<String, String> {
    match format {
        ExportFormat::Json => serde_json::to_string_pretty(config)
            .map_err(|e| format!("序列化失败: {}", e)),
        ExportFormat::Hosts => {
            let active_id = config.active_profile_id.as_ref().ok_or("无激活的 profile")?;
            let profile = config
                .profile(active_id)
                .ok_or("激活的 profile 不存在")?;
            Ok(profile.content.clone())
        }
    }
}

/// 导入：解析内容并合并到新 Config
/// - Hosts：作为原始文本创建一个新 profile
/// - Json：直接替换整个 Config
pub fn import_config(
    content: &str,
    format: &ExportFormat,
) -> Result<(Config, ImportSummary), String> {
    match format {
        ExportFormat::Json => {
            let config: Config = serde_json::from_str(content)
                .map_err(|e| format!("解析 JSON 失败: {}", e))?;
            let profile_count = config.profiles.len();
            // 统计所有 profile 的行数
            let entry_count: usize = config
                .profiles
                .iter()
                .map(|p| p.content.lines().count())
                .sum();
            Ok((
                config,
                ImportSummary {
                    profile_count,
                    entry_count,
                },
            ))
        }
        ExportFormat::Hosts => {
            let now = chrono::Utc::now().to_rfc3339();
            let profile = crate::models::Profile {
                id: uuid::Uuid::new_v4().to_string(),
                name: format!("导入 {}", &now[..10]),
                content: content.to_string(),
                enabled: false,
                created_at: now,
                last_applied_at: None,
                is_remote: false,
                url: None,
                last_fetch_at: None,
                auto_refresh_secs: 0,
            };
            let entry_count = profile.content.lines().count();
            let pid = profile.id.clone();
            let config = Config {
                profiles: vec![profile],
                active_profile_id: Some(pid),
                backups: vec![],
            };
            Ok((
                config,
                ImportSummary {
                    profile_count: 1,
                    entry_count,
                },
            ))
        }
    }
}

//! 应用配置持久化（数据目录可自定义，默认 ~/.SetHosts）

use crate::models::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 应用级别设置（始终存于 app_data_dir/settings.json）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    /// 自定义数据目录（None 则使用默认 ~/.SetHosts）
    pub custom_data_dir: Option<String>,
    /// 语言：zh-CN / en
    #[serde(default = "default_lang")]
    pub language: String,
    /// 主题：light / dark
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 启动时隐藏窗口
    #[serde(default)]
    pub hide_on_startup: bool,
    /// 是否使用 HTTP 代理拉取远程 hosts
    #[serde(default)]
    pub proxy_enabled: bool,
    /// 代理协议：http / https / socks5
    #[serde(default = "default_proxy_protocol")]
    pub proxy_protocol: String,
    /// 代理主机
    #[serde(default)]
    pub proxy_host: String,
    /// 代理端口（0 = 未设置）
    #[serde(default)]
    pub proxy_port: u16,
    /// 启动时自动刷新已启用的远程 hosts
    #[serde(default = "default_true")]
    pub remote_auto_refresh: bool,
    /// 写入系统 hosts 的模式：append（托管块追加，保留原有条目）/ overwrite（完全替换）
    #[serde(default = "default_write_mode")]
    pub write_mode: String,
    /// 内置 DNS 服务器是否随应用启动自动运行。
    ///
    /// 已废弃：该开关的配置界面已从所有端移除——移动端无条件启动（它是移动端让 hosts
    /// 映射生效的唯一免 root 途径），桌面端无条件不启动。保留字段只为兼容旧版
    /// settings.json，读写都不再影响行为。
    #[serde(default)]
    pub dns_proxy_auto_start: bool,
    /// 内置 DNS 服务器监听端口（0 = 使用默认端口）
    #[serde(default)]
    pub dns_proxy_port: u16,
    /// 内置 DNS 服务器上游 DNS（逗号分隔；空 = 自动探测系统 DNS）
    #[serde(default)]
    pub dns_upstream: String,
}

fn default_lang() -> String {
    "en".to_string()
}

fn default_theme() -> String {
    "light".to_string()
}

fn default_proxy_protocol() -> String {
    "http".to_string()
}

fn default_true() -> bool {
    true
}

fn default_write_mode() -> String {
    "append".to_string()
}

/// settings.json 路径（始终在 app_data_dir 下，保证可找到）
fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取 app_data_dir 失败: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建设置目录失败: {}", e))?;
    Ok(dir.join("settings.json"))
}

/// 读取应用设置
pub fn load_settings_public(app: &AppHandle) -> AppSettings {
    match settings_path(app) {
        Ok(path) if path.exists() => {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        }
        _ => AppSettings::default(),
    }
}

fn load_settings(app: &AppHandle) -> AppSettings {
    load_settings_public(app)
}

/// 保存应用设置
pub fn save_settings_public(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化设置失败: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("写入设置失败: {}", e))?;
    Ok(())
}

/// 获取当前数据目录（自定义或默认 ~/.SetHosts）
pub fn get_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let settings = load_settings(app);
    if let Some(custom) = settings.custom_data_dir {
        let path = PathBuf::from(&custom);
        std::fs::create_dir_all(&path).map_err(|e| format!("创建数据目录失败: {}", e))?;
        return Ok(path);
    }
    // 默认: ~/.SetHosts；Android 等平台无用户主目录时回退到系统 app_data_dir
    let dir = match dirs::home_dir() {
        Some(home) => home.join(".SetHosts"),
        None => app
            .path()
            .app_data_dir()
            .map_err(|e| format!("获取数据目录失败: {}", e))?,
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {}", e))?;
    Ok(dir)
}

/// 更改数据目录：迁移 config.json 到新位置
pub fn change_data_dir(app: &AppHandle, new_dir: &str) -> Result<(), String> {
    let new_path = PathBuf::from(new_dir);
    std::fs::create_dir_all(&new_path).map_err(|e| format!("创建新目录失败: {}", e))?;

    // 复制旧 config.json 到新位置
    let old_config = config_path(app)?;
    let new_config = new_path.join("config.json");
    if old_config.exists() {
        let content = std::fs::read(&old_config)
            .map_err(|e| format!("读取旧配置失败: {}", e))?;
        std::fs::write(&new_config, content)
            .map_err(|e| format!("写入新配置失败: {}", e))?;
    }

    // 更新 settings
    let mut settings = load_settings(app);
    settings.custom_data_dir = Some(new_dir.to_string());
    save_settings_public(app, &settings)?;

    Ok(())
}

/// 配置文件路径（基于当前数据目录）
pub fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = get_data_dir(app)?;
    Ok(dir.join("config.json"))
}

/// 加载配置，不存在则返回默认
pub fn load_config(app: &AppHandle) -> Result<Config, String> {
    let path = config_path(app)?;
    if !path.exists() {
        let config = Config::default();
        save_config(app, &config)?;
        return Ok(config);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;
    Ok(config)
}

/// 保存配置
pub fn save_config(app: &AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

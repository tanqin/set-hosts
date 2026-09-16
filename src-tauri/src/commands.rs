//! Tauri 命令层：所有 #[tauri::command] 函数
//!
//! SwitchHosts 风格：每个 profile 存储原始 hosts 文本，
//! 启用的 profile 的内容会被合并写入系统 hosts 的「托管块」。

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tauri::State;

use crate::dns_proxy::{self, DnsProxy};
use crate::models::{
    BackupRecord, Config, ExportFormat, HostEntry, ImportSummary, Profile, ProxyStatus,
};

/// 应用全局状态
pub struct AppState {
    pub config: Mutex<Config>,
    /// DNS 代理状态（移动端让 hosts 映射生效；桌面端可用于调试）
    pub proxy: Arc<DnsProxy>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Mutex::new(config),
            proxy: DnsProxy::shared(),
        }
    }
}

/// 用当前配置（所有启用 profile）重建 DNS 代理映射表，返回映射条数
///
/// 配置变更（启停 profile、编辑内容、删除、刷新远程 hosts、导入）后都应调用，
/// 保证 DNS 代理与系统 hosts 的映射始终一致。
pub fn refresh_proxy_mappings(state: &AppState) -> usize {
    let mappings = match state.config.lock() {
        Ok(cfg) => dns_proxy::mappings_from_config(&cfg),
        Err(e) => {
            log::warn!("重建 DNS 代理映射失败: {}", e);
            return 0;
        }
    };
    state.proxy.set_mappings(mappings)
}

// ============ Profile 管理 ============

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(cfg.profiles.clone())
}

#[tauri::command]
pub fn get_active_profile(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(cfg.active_profile_id.clone())
}

#[tauri::command]
pub fn set_active_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    if cfg.profile(&profile_id).is_none() {
        return Err("profile 不存在".to_string());
    }
    cfg.active_profile_id = Some(profile_id);
    crate::store::save_config(&app, &cfg)
}

#[tauri::command]
pub fn create_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<Profile, String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        content: String::new(),
        enabled: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_applied_at: None,
        is_remote: false,
        url: None,
        last_fetch_at: None,
        auto_refresh_secs: 0,
    };
    cfg.profiles.push(profile.clone());
    crate::store::save_config(&app, &cfg)?;
    Ok(profile)
}

#[tauri::command]
pub async fn delete_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    // 先检查该 profile 是否启用
    let was_enabled = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.profile(&id).map(|p| p.enabled).unwrap_or(false)
    };

    // 从配置中删除
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        if cfg.profiles.len() <= 1 {
            return Err("至少保留一个 profile".to_string());
        }
        cfg.profiles.retain(|p| p.id != id);
        if cfg.active_profile_id.as_deref() == Some(id.as_str()) {
            cfg.active_profile_id = cfg.profiles.first().map(|p| p.id.clone());
        }
        crate::store::save_config(&app, &cfg)?;
    }

    // 配置已变化：同步 DNS 代理映射（移除被删 profile 的条目）
    refresh_proxy_mappings(&state);

    // 若被删除的 profile 处于启用状态，重新写入系统 hosts（移除其条目）
    if was_enabled && crate::hosts_path::is_desktop() {
        let merged = build_merged_hosts(&app, &state)?;
        tauri::async_runtime::spawn_blocking(move || write_hosts(merged))
            .await
            .map_err(|e| format!("应用失败: {}", e))??;
    }

    Ok(())
}

#[tauri::command]
pub fn rename_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let profile = cfg.profile_mut(&id).ok_or("profile 不存在")?;
    profile.name = name;
    crate::store::save_config(&app, &cfg)
}

#[tauri::command]
pub fn get_profile_content(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<String, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let profile = cfg.profile(&profile_id).ok_or("profile 不存在")?;
    Ok(profile.content.clone())
}

#[tauri::command]
pub fn save_profile_content(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    content: String,
) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let profile = cfg.profile_mut(&profile_id).ok_or("profile 不存在")?;
    profile.content = content;
    crate::store::save_config(&app, &cfg)?;
    drop(cfg);

    // 内容变更后同步 DNS 代理映射（若该 profile 已启用，新条目立即生效）
    refresh_proxy_mappings(&state);
    Ok(())
}

/// 切换 profile 启用状态：
/// - 启用：将内容加入系统 hosts 托管块
/// - 禁用：从系统 hosts 托管块移除
/// 托管块 = 所有启用 profile 内容的并集
#[tauri::command]
pub async fn toggle_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<String, String> {
    let enabled;
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        let profile = cfg.profile_mut(&profile_id).ok_or("profile 不存在")?;
        profile.enabled = !profile.enabled;
        enabled = profile.enabled;
        let now = chrono::Utc::now().to_rfc3339();
        for p in cfg.profiles.iter_mut() {
            if p.enabled {
                p.last_applied_at = Some(now.clone());
            }
        }
        crate::store::save_config(&app, &cfg)?;
    } // 锁在此处释放

    refresh_proxy_mappings(&state);

    if !crate::hosts_path::is_desktop() {
        return Ok(format!(
            "已{}（移动端请开启 DNS 代理）",
            if enabled { "启用" } else { "禁用" }
        ));
    }

    // 提权写入系统 hosts 属于阻塞操作，放到独立线程避免卡 UI
    let merged = build_merged_hosts(&app, &state)?;
    tauri::async_runtime::spawn_blocking(move || write_hosts(merged))
        .await
        .map_err(|e| format!("应用失败: {}", e))??;
    Ok(format!("已{}", if enabled { "启用" } else { "禁用" }))
}

/// 显式应用某个 profile（设为激活 + 启用 + 写入）
#[tauri::command]
pub async fn apply_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<String, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.active_profile_id = Some(profile_id.clone());
        let profile = cfg.profile_mut(&profile_id).ok_or("profile 不存在")?;
        profile.enabled = true;
        profile.last_applied_at = Some(chrono::Utc::now().to_rfc3339());
        crate::store::save_config(&app, &cfg).ok();
    }

    refresh_proxy_mappings(&state);

    if !crate::hosts_path::is_desktop() {
        return Ok("已启用，请在设置中开启 DNS 代理使映射生效".to_string());
    }

    let merged = build_merged_hosts(&app, &state)?;
    tauri::async_runtime::spawn_blocking(move || write_hosts(merged))
        .await
        .map_err(|e| format!("应用失败: {}", e))??;
    Ok("已应用".to_string())
}

/// 构建所有启用 profile 合并后的 hosts 文本（含备份），不执行写入
fn build_merged_hosts(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
) -> Result<String, String> {
    // 读取当前系统 hosts（一次读取，同时用于备份和合并）
    let current = crate::privilege::read_hosts_content().unwrap_or_default();

    // 备份当前 hosts
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        if !current.is_empty() {
            let backup = BackupRecord {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                content: current.clone(),
                source_path: crate::hosts_path::hosts_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            cfg.backups.push(backup);
            if cfg.backups.len() > 50 {
                let drop_from = cfg.backups.len() - 50;
                cfg.backups.drain(0..drop_from);
            }
        }
        crate::store::save_config(app, &cfg).ok();
    }

    // 收集所有启用 profile 的条目
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let mut all_entries: Vec<HostEntry> = Vec::new();
    for p in cfg.profiles.iter().filter(|p| p.enabled) {
        let mut entries = crate::parser::parse_hosts_text(&p.content);
        all_entries.append(&mut entries);
    }
    drop(cfg);

    // 按写入模式生成内容：
    // - append：托管块追加到当前 hosts 末尾，保留系统原有条目
    // - overwrite：仅托管块整体替换系统 hosts（备份已在上方创建）
    let settings = crate::store::load_settings_public(app);
    if settings.write_mode == "overwrite" {
        Ok(crate::parser::build_overwrite_content(&all_entries))
    } else {
        Ok(crate::parser::merge_managed_block(&current, &all_entries))
    }
}

/// 将合并内容写入系统 hosts（阻塞操作，应在 spawn_blocking 中调用）
fn write_hosts(merged: String) -> Result<(), String> {
    crate::privilege::write_hosts_elevated(&merged)?;
    // DNS 刷新放后台执行，不阻塞返回
    tauri::async_runtime::spawn_blocking(|| {
        let _ = crate::dns_flush::flush_dns_cache();
    });
    Ok(())
}

// ============ 备份与还原 ============

#[tauri::command]
pub fn backup_hosts(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupRecord, String> {
    // 移动端无法读取系统 hosts，创建空备份没有意义
    if !crate::hosts_path::is_desktop() {
        return Err("移动端不支持备份系统 hosts".to_string());
    }
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let content = crate::privilege::read_hosts_content().unwrap_or_default();
    let source_path = crate::hosts_path::hosts_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let record = BackupRecord {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        content,
        source_path,
    };
    cfg.backups.push(record.clone());
    if cfg.backups.len() > 50 {
        let drop_from = cfg.backups.len() - 50;
        cfg.backups.drain(0..drop_from);
    }
    crate::store::save_config(&app, &cfg)?;
    Ok(record)
}

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupRecord>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let mut backups = cfg.backups.clone();
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(backups)
}

#[tauri::command]
pub async fn restore_backup(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    backup_id: String,
) -> Result<String, String> {
    let content = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let backup = cfg
            .backups
            .iter()
            .find(|b| b.id == backup_id)
            .ok_or("备份不存在")?;
        backup.content.clone()
    };

    if crate::hosts_path::is_desktop() {
        tauri::async_runtime::spawn_blocking(move || write_hosts(content))
            .await
            .map_err(|e| format!("还原失败: {}", e))??;
        Ok("已还原备份".to_string())
    } else {
        Err("移动端还原请通过 DNS 代理重载映射".to_string())
    }
}

// ============ 读取当前 hosts ============

#[tauri::command]
pub fn get_current_hosts_content() -> Result<String, String> {
    if !crate::hosts_path::is_desktop() {
        return Err("移动端无法直接读取系统 hosts".to_string());
    }
    crate::privilege::read_hosts_content()
}

// ============ 导入导出 ============

#[tauri::command]
pub fn export_config(
    state: State<'_, AppState>,
    format: ExportFormat,
) -> Result<String, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    crate::import_export::export_config(&cfg, &format)
}

#[tauri::command]
pub fn import_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    content: String,
    format: ExportFormat,
) -> Result<ImportSummary, String> {
    apply_import(app, state, content, format)
}

/// 导出到指定文件
#[tauri::command]
pub fn export_config_to_file(
    state: State<'_, AppState>,
    path: String,
    format: ExportFormat,
) -> Result<(), String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let content = crate::import_export::export_config(&cfg, &format)?;
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {}", e))
}

/// 从文件导入
#[tauri::command]
pub fn import_config_from_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    format: ExportFormat,
) -> Result<ImportSummary, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))?;
    apply_import(app, state, content, format)
}

fn apply_import(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    content: String,
    format: ExportFormat,
) -> Result<ImportSummary, String> {
    let (new_config, summary) = crate::import_export::import_config(&content, &format)?;
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    *cfg = new_config;
    crate::store::save_config(&app, &cfg)?;
    drop(cfg);

    // 导入会整体替换配置：同步 DNS 代理映射
    refresh_proxy_mappings(&state);
    Ok(summary)
}

// ============ DNS 代理 ============
//
// 内置本地 DNS 服务器，把启用 profile 的映射直接应答，未命中的域名转发上游。
// 移动端（Android/iOS）无法写入系统 hosts，靠它让映射生效（VPN 隧道接管系统 DNS）；
// 桌面端也可手动启动，便于验证映射结果。

/// 启动 DNS 代理（已在运行时按新参数重启），并记住端口 / 上游设置
#[tauri::command]
pub async fn start_dns_proxy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    port: Option<u16>,
    upstream: Option<String>,
) -> Result<ProxyStatus, String> {
    let mut settings = crate::store::load_settings_public(&app);
    if let Some(p) = port {
        settings.dns_proxy_port = p;
    }
    if let Some(u) = upstream {
        settings.dns_upstream = u.trim().to_string();
    }
    crate::store::save_settings_public(&app, &settings)?;

    // 启动前先按当前配置重建映射，保证首次查询即命中
    let mapping_count = refresh_proxy_mappings(&state);

    let port = if settings.dns_proxy_port == 0 {
        dns_proxy::DEFAULT_PORT
    } else {
        settings.dns_proxy_port
    };
    let upstreams = dns_proxy::resolver::resolve_upstreams(&settings.dns_upstream);
    if upstreams.is_empty() {
        return Err("未能确定上游 DNS 服务器，请在选项中手动填写".to_string());
    }

    let bind = SocketAddr::from(([127, 0, 0, 1], port));
    state.proxy.start(bind, upstreams).await?;
    log::info!("DNS 代理已启动，映射 {} 条", mapping_count);

    // 移动端：尝试通过 VPN 隧道把系统 DNS 指向本地服务器
    if crate::hosts_path::is_mobile() {
        if let Err(e) = crate::mobile::start_tunnel(port) {
            log::warn!("启动 VPN 隧道失败: {}", e);
        }
    }

    Ok(state.proxy.status())
}

/// 停止 DNS 代理
#[tauri::command]
pub async fn stop_dns_proxy(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if crate::hosts_path::is_mobile() {
        if let Err(e) = crate::mobile::stop_tunnel() {
            log::warn!("停止 VPN 隧道失败: {}", e);
        }
    }
    state.proxy.stop().await
}

/// 查询 DNS 代理运行状态
#[tauri::command]
pub fn get_proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    Ok(state.proxy.status())
}

/// 启动时按设置自动运行 DNS 代理（失败仅记录日志，不影响应用启动）
pub async fn auto_start_dns_proxy(app: tauri::AppHandle) {
    use tauri::Manager;

    let settings = crate::store::load_settings_public(&app);
    if !settings.dns_proxy_auto_start {
        return;
    }

    let state = app.state::<AppState>();
    let mapping_count = refresh_proxy_mappings(&state);

    let port = if settings.dns_proxy_port == 0 {
        dns_proxy::DEFAULT_PORT
    } else {
        settings.dns_proxy_port
    };
    let upstreams = dns_proxy::resolver::resolve_upstreams(&settings.dns_upstream);
    if upstreams.is_empty() {
        log::warn!("DNS 代理未自动启动：未能确定上游 DNS 服务器");
        return;
    }

    let bind = SocketAddr::from(([127, 0, 0, 1], port));
    match state.proxy.start(bind, upstreams).await {
        Ok(addr) => log::info!("DNS 代理已自动启动: {}（映射 {} 条）", addr, mapping_count),
        Err(e) => log::error!("DNS 代理自动启动失败: {}", e),
    }
}

// ============ 平台信息 ============

#[tauri::command]
pub fn get_platform_info() -> serde_json::Value {
    serde_json::json!({
        "is_desktop": crate::hosts_path::is_desktop(),
        "is_mobile": crate::hosts_path::is_mobile(),
        "os": std::env::consts::OS,
        "hosts_path": crate::hosts_path::hosts_path().map(|p| p.to_string_lossy().to_string()),
    })
}

// ============ 打开 hosts 所在文件夹 ============

#[tauri::command]
pub fn open_hosts_folder() -> Result<(), String> {
    let hosts_path = crate::hosts_path::hosts_path()
        .ok_or("无法获取 hosts 文件路径")?;
    let parent = hosts_path
        .parent()
        .ok_or("无法获取 hosts 文件所在目录")?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(parent.as_os_str())
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    Ok(())
}

// ============ 数据目录管理 ============

#[tauri::command]
pub fn get_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    let dir = crate::store::get_data_dir(&app)?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn change_data_dir(app: tauri::AppHandle, new_dir: String) -> Result<(), String> {
    crate::store::change_data_dir(&app, &new_dir)
}

// ============ 应用设置 ============

#[tauri::command]
pub fn get_app_settings(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let settings = crate::store::load_settings_public(&app);
    serde_json::to_value(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_app_settings(
    app: tauri::AppHandle,
    language: Option<String>,
    theme: Option<String>,
    hide_on_startup: Option<bool>,
    proxy_enabled: Option<bool>,
    proxy_protocol: Option<String>,
    proxy_host: Option<String>,
    proxy_port: Option<u16>,
    remote_auto_refresh: Option<bool>,
    write_mode: Option<String>,
    dns_proxy_auto_start: Option<bool>,
    dns_proxy_port: Option<u16>,
    dns_upstream: Option<String>,
) -> Result<(), String> {
    let mut settings = crate::store::load_settings_public(&app);
    if let Some(lang) = language {
        settings.language = lang;
    }
    if let Some(t) = theme {
        settings.theme = t;
    }
    if let Some(h) = hide_on_startup {
        settings.hide_on_startup = h;
    }
    if let Some(v) = proxy_enabled {
        settings.proxy_enabled = v;
    }
    if let Some(v) = proxy_protocol {
        let v = v.to_lowercase();
        if matches!(v.as_str(), "http" | "https" | "socks5") {
            settings.proxy_protocol = v;
        }
    }
    if let Some(v) = proxy_host {
        settings.proxy_host = v.trim().to_string();
    }
    if let Some(v) = proxy_port {
        settings.proxy_port = v;
    }
    if let Some(v) = remote_auto_refresh {
        settings.remote_auto_refresh = v;
    }
    if let Some(v) = write_mode {
        let v = v.to_lowercase();
        if matches!(v.as_str(), "append" | "overwrite") {
            settings.write_mode = v;
        }
    }
    if let Some(v) = dns_proxy_auto_start {
        settings.dns_proxy_auto_start = v;
    }
    if let Some(v) = dns_proxy_port {
        settings.dns_proxy_port = v;
    }
    if let Some(v) = dns_upstream {
        settings.dns_upstream = v.trim().to_string();
    }
    crate::store::save_settings_public(&app, &settings)
}

// ============ 开机自启（桌面端，tauri-plugin-autostart） ============

/// 查询开机自启是否已开启（移动端恒为 false）
#[tauri::command]
pub fn get_autostart_status(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        return app.autolaunch().is_enabled().map_err(|e| e.to_string());
    }
    #[cfg(not(desktop))]
    {
        let _ = &app;
        Ok(false)
    }
}

/// 开启 / 关闭开机自启（移动端为空操作）
#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app.autolaunch();
        return if enabled {
            autostart.enable()
        } else {
            autostart.disable()
        }
        .map_err(|e| e.to_string());
    }
    #[cfg(not(desktop))]
    {
        let _ = (&app, enabled);
        Ok(())
    }
}

// ============ 远程 Hosts（经代理拉取，SwitchHosts 风格） ============

/// 构建带可选代理的 HTTP 客户端（按设置中的代理配置）
fn build_http_client(app: &tauri::AppHandle) -> Result<reqwest::Client, String> {
    let settings = crate::store::load_settings_public(app);
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("SetHosts/", env!("CARGO_PKG_VERSION")));

    if settings.proxy_enabled && !settings.proxy_host.is_empty() && settings.proxy_port != 0 {
        let url = format!(
            "{}://{}:{}",
            settings.proxy_protocol, settings.proxy_host, settings.proxy_port
        );
        let proxy = reqwest::Proxy::all(&url).map_err(|e| format!("代理配置无效: {}", e))?;
        builder = builder.proxy(proxy);
    }

    builder.build().map_err(|e| format!("创建 HTTP 客户端失败: {}", e))
}

/// 拉取远程 hosts 文本（走配置的代理），限制 8MB
async fn fetch_remote_content(app: &tauri::AppHandle, url: &str) -> Result<String, String> {
    let client = build_http_client(app)?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("内容超过 8MB 限制".to_string());
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// 新增远程 hosts profile，创建后立即拉取一次内容
/// `auto_refresh_secs`：自动刷新间隔（秒），0 或 None = 从不自动刷新
#[tauri::command]
pub async fn create_remote_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
    url: String,
    auto_refresh_secs: Option<u64>,
) -> Result<Profile, String> {
    let url = url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("URL 必须以 http:// 或 https:// 开头".to_string());
    }

    // 先拉取（不持锁，走网络）
    let content = fetch_remote_content(&app, &url).await?;

    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        content,
        enabled: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_applied_at: None,
        is_remote: true,
        url: Some(url),
        last_fetch_at: Some(chrono::Utc::now().to_rfc3339()),
        auto_refresh_secs: auto_refresh_secs.unwrap_or(0),
    };
    cfg.profiles.push(profile.clone());
    crate::store::save_config(&app, &cfg)?;
    Ok(profile)
}

/// 立即刷新远程 hosts：拉取最新内容；若该 profile 已启用则同时重新写入系统 hosts
#[tauri::command]
pub async fn refresh_remote_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Profile, String> {
    // 取 URL（短锁）
    let (url, was_enabled) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let p = cfg.profile(&profile_id).ok_or("profile 不存在")?;
        if !p.is_remote {
            return Err("该 profile 不是远程 hosts".to_string());
        }
        (p.url.clone().ok_or("远程 hosts 缺少 URL")?, p.enabled)
    };

    // 拉取（不持锁）
    let content = fetch_remote_content(&app, &url).await?;

    // 写回
    let profile = {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        let p = cfg.profile_mut(&profile_id).ok_or("profile 不存在")?;
        p.content = content;
        p.last_fetch_at = Some(chrono::Utc::now().to_rfc3339());
        let cloned = p.clone();
        crate::store::save_config(&app, &cfg)?;
        cloned
    };

    // 内容已更新：同步 DNS 代理映射
    refresh_proxy_mappings(&state);

    // 已启用的远程 hosts 刷新后重新应用
    if was_enabled && crate::hosts_path::is_desktop() {
        let merged = build_merged_hosts(&app, &state)?;
        tauri::async_runtime::spawn_blocking(move || write_hosts(merged))
            .await
            .map_err(|e| format!("应用失败: {}", e))??;
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        if let Some(p) = cfg.profile_mut(&profile_id) {
            p.last_applied_at = Some(chrono::Utc::now().to_rfc3339());
            crate::store::save_config(&app, &cfg).ok();
        }
    }

    Ok(profile)
}

/// 启动时自动刷新所有已启用的远程 hosts（后台执行，失败仅记日志）
pub async fn auto_refresh_remote_profiles(app: tauri::AppHandle) {
    use tauri::Manager;

    let settings = crate::store::load_settings_public(&app);
    if !settings.remote_auto_refresh {
        return;
    }

    let state = app.state::<AppState>();
    let ids: Vec<String> = {
        let cfg = match state.config.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        cfg.profiles
            .iter()
            .filter(|p| p.is_remote && p.enabled)
            .map(|p| p.id.clone())
            .collect()
    };

    for id in ids {
        if let Err(e) = refresh_remote_profile(app.clone(), state.clone(), id).await {
            log::warn!("启动自动刷新远程 hosts 失败: {}", e);
        }
    }
}

/// 解析 RFC3339 时间为 UTC
fn parse_utc_time(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc))
}

/// 远程 hosts 定时自动刷新：每 30 秒轮询一次，按各 profile 自身的间隔触发刷新
/// （间隔为 0 表示从不自动刷新；刷新成功后向前端广播 remote-hosts-refreshed 事件）
pub async fn spawn_remote_auto_refresh_loop(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};

    // 轮询粒度（秒）
    const TICK_SECS: u64 = 30;

    // 记录每个 profile 上次尝试刷新的时间，避免拉取失败时按轮询粒度反复重试
    let mut last_attempt: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>> =
        std::collections::HashMap::new();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(TICK_SECS)).await;

        let state = app.state::<AppState>();
        let now = chrono::Utc::now();

        let (remote_ids, due_ids): (Vec<String>, Vec<String>) = {
            let cfg = match state.config.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let mut remote_ids = Vec::new();
            let mut due_ids = Vec::new();
            for p in cfg.profiles.iter() {
                if !p.is_remote {
                    continue;
                }
                remote_ids.push(p.id.clone());
                if p.auto_refresh_secs == 0 {
                    continue;
                }
                let interval = chrono::Duration::seconds(p.auto_refresh_secs as i64);
                let last = last_attempt
                    .get(&p.id)
                    .copied()
                    .or_else(|| p.last_fetch_at.as_deref().and_then(parse_utc_time));
                // 无拉取记录时立即刷新一次
                match last {
                    Some(t) => {
                        if now.signed_duration_since(t) >= interval {
                            due_ids.push(p.id.clone());
                        }
                    }
                    None => due_ids.push(p.id.clone()),
                }
            }
            (remote_ids, due_ids)
        };

        // 丢弃已删除 profile 的记录
        last_attempt.retain(|id, _| remote_ids.contains(id));

        for id in due_ids {
            last_attempt.insert(id.clone(), chrono::Utc::now());
            match refresh_remote_profile(app.clone(), state.clone(), id.clone()).await {
                Ok(profile) => {
                    // 后台刷新发生在命令调用之外，通知前端同步显示
                    let _ = app.emit("remote-hosts-refreshed", &profile);
                }
                Err(e) => log::warn!("定时自动刷新远程 hosts 失败 ({})：{}", id, e),
            }
        }
    }
}

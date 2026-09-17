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
    /// 内置 DNS 服务器状态（仅移动端使用：配合 VPN 隧道让 hosts 映射生效）
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
        Ok(cfg) => {
            // 所有 profile 里的域名（含未启用的）都登记为「本应用管理过的域名」：
            // 它们的应答不能被客户端长期缓存，否则开关配置会「改了却还是旧的」，
            // 移动端表现为「关了再开就访问失败、要重启应用」（见 DnsProxy::managed）
            state
                .proxy
                .add_managed_domains(dns_proxy::managed_domains_from_config(&cfg));
            dns_proxy::mappings_from_config(&cfg)
        }
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
        // 移动端：映射不写系统 hosts，而是由内置 DNS 服务器 + VPN 隧道生效。
        // 这里跟随「配置开关」同步隧道——打开时申请系统 VPN 授权并接管系统 DNS
        // （用户拒绝过也会重新弹，因为是用户主动打开），全部关闭时收起隧道。
        if let Err(reason) = sync_mobile_tunnel(&app, &state, enabled).await {
            // 开启失败（本地 DNS 服务器起不来）时把开关回滚：界面停在「已启用」
            // 而映射实际不生效，只会让用户以为是自己用错了，而且再也查不出原因。
            if enabled {
                rollback_profile_enabled(&app, &state, &profile_id);
            }
            return Err(reason);
        }
        return Ok(format!("已{}", if enabled { "启用" } else { "禁用" }));
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
        // 移动端：与「配置开关」一致地同步 VPN 隧道（见 toggle_profile），
        // 失败同样回滚，别让界面显示「已启用」而映射实际没生效
        if let Err(reason) = sync_mobile_tunnel(&app, &state, true).await {
            rollback_profile_enabled(&app, &state, &profile_id);
            return Err(reason);
        }
        return Ok("已启用".to_string());
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
    let addr = state.proxy.start(bind, upstreams).await?;
    log::info!("DNS 代理已启动: {}，映射 {} 条", addr, mapping_count);

    // 移动端：尝试通过 VPN 隧道把系统 DNS 指向本地服务器。
    // 端口用**实际监听**的那个：首选端口被占用时服务器会退到系统随机端口，
    // 这里若还用配置里的端口，隧道就指向了一个没人应答的端口（全机 DNS 超时）。
    if crate::hosts_path::is_mobile() {
        if let Err(e) = crate::mobile::start_tunnel(addr.port()) {
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

/// 是否存在处于启用状态的 profile（只有它们的内容会进入映射表）
fn has_enabled_profile(state: &AppState) -> bool {
    state
        .config
        .lock()
        .map(|cfg| cfg.profiles.iter().any(|p| p.enabled))
        .unwrap_or(false)
}

/// 本地 DNS 服务器**真正在监听**的端口；没在运行则返回 `None`
///
/// 这里刻意不回退到默认端口：VPN 隧道会把整台设备的 DNS 都指向这个地址，
/// 只要那个端口上没人应答，全机所有域名解析都会超时——表现就是「信号、流量都正常，
/// 但每个应用都上不了网，直接访问 IP 却可以」。这比暂不接管系统 DNS 糟糕得多，
/// 所以「端口不确定」时唯一正确的做法是不建立隧道。
fn live_dns_port(state: &AppState) -> Option<u16> {
    state
        .proxy
        .status()
        .listen_addr
        .as_deref()
        .and_then(|addr| addr.rsplit(':').next())
        .and_then(|port| port.parse::<u16>().ok())
}

/// 确保本地 DNS 服务器在运行：没跑就按设置里的参数拉起来，返回它实际监听的端口
///
/// 接管系统 DNS 之前必须先过这一关，否则会建出一条指向空端口的隧道（见 [`live_dns_port`]）。
async fn ensure_local_dns_running(app: &tauri::AppHandle, state: &AppState) -> Option<u16> {
    if let Some(port) = live_dns_port(state) {
        return Some(port);
    }

    let settings = crate::store::load_settings_public(app);
    let port = if settings.dns_proxy_port == 0 {
        dns_proxy::DEFAULT_PORT
    } else {
        settings.dns_proxy_port
    };
    let upstreams = dns_proxy::resolver::resolve_upstreams(&settings.dns_upstream);

    match state
        .proxy
        .start(SocketAddr::from(([127, 0, 0, 1], port)), upstreams)
        .await
    {
        // 以实际绑定结果为准：首选端口被占用时会退到系统随机端口
        Ok(_) => {
            let actual = live_dns_port(state);
            match actual {
                Some(port) => log::info!("本地 DNS 服务器已按需启动: 127.0.0.1:{}", port),
                None => log::error!("本地 DNS 服务器启动后仍未监听"),
            }
            actual
        }
        // 失败原因已记进 proxy（失败路径会写 last_error），这里不再重复刷日志
        Err(_) => None,
    }
}

/// 本地 DNS 服务器启动失败的**可读原因**（供前端提示与诊断报告）
fn local_dns_failure_reason(state: &AppState) -> String {
    state
        .proxy
        .last_error()
        .unwrap_or_else(|| "未能确定上游 DNS 服务器（请在选项里填写有效上游）".to_string())
}

/// 移动端开启配置失败时，把开关回滚成关闭状态
///
/// 移动端「开启配置」的全部意义就是让映射生效，而这必须依赖本地 DNS 服务器；
/// 服务器起不来时开关停在「已启用」纯属误导，用户还会以为是 hosts 内容写错了。
fn rollback_profile_enabled(app: &tauri::AppHandle, state: &AppState, profile_id: &str) {
    if let Ok(mut cfg) = state.config.lock() {
        if let Some(profile) = cfg.profile_mut(profile_id) {
            profile.enabled = false;
            profile.last_applied_at = None;
        }
        let _ = crate::store::save_config(app, &cfg);
    }
    refresh_proxy_mappings(state);
}

/// 移动端：按「配置开关」同步 VPN 隧道（桌面端空操作）
///
/// 移动端写不了系统 hosts，映射只能靠「内置 DNS 服务器 + VPN 隧道」生效，所以开关
/// 打开时要申请系统 VPN 授权并接管系统 DNS（用户主动打开，即使拒绝过也会重新弹窗）；
/// 关闭后若已无任何启用的配置，则收起隧道，免得系统一直挂着 VPN。
///
/// 返回 `Err` 表示**这次开启并没有真正让映射生效**（本地 DNS 服务器起不来），
/// 调用方必须据此回滚开关——否则界面显示「已启用」、实际什么都没发生。
async fn sync_mobile_tunnel(
    app: &tauri::AppHandle,
    state: &AppState,
    enabled: bool,
) -> Result<(), String> {
    if !crate::hosts_path::is_mobile() {
        return Ok(());
    }
    if enabled {
        // 先把本地 DNS 服务器拉起来再接管：隧道一旦建立，整机 DNS 都交给它，
        // 指向一个没人监听的端口等于让全机断域名解析
        let Some(port) = ensure_local_dns_running(app, state).await else {
            let reason = local_dns_failure_reason(state);
            log::warn!("本地 DNS 服务器不可用，本次不接管系统 DNS: {}", reason);
            return Err(format!("本地 DNS 服务器未能启动，映射无法生效：{reason}"));
        };
        log::info!("移动端：请求接管系统 DNS，隧道转发到 127.0.0.1:{}", port);
        if let Err(e) = crate::mobile::start_tunnel(port) {
            log::warn!("申请系统 VPN 授权失败: {}", e);
            return Err(format!("申请系统 VPN 授权失败：{e}"));
        }
        return Ok(());
    }
    if !has_enabled_profile(state) {
        if let Err(e) = crate::mobile::stop_tunnel() {
            log::warn!("收起 VPN 隧道失败: {}", e);
        }
    }
    Ok(())
}

/// 移动端自愈：确保 VPN 隧道已接管系统 DNS。
///
/// 应用回到前台时调用。只在「有配置处于启用状态」且本地 DNS 服务器正在运行时才接管；
/// 已授权过 VPN 时不会弹任何弹窗，用户曾拒绝过也不会
/// （见 [`crate::mobile::auto_start_tunnel`]）。
/// 返回值为当前隧道是否已接管系统 DNS，桌面端恒为 false。
#[tauri::command]
pub async fn ensure_tunnel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    if !crate::hosts_path::is_mobile() {
        return Ok(false);
    }
    // 没有任何启用的配置时不接管：用户可能刚把开关全部关掉
    if !has_enabled_profile(state.inner()) {
        return Ok(false);
    }

    // 回到前台是自愈时机：本地 DNS 服务器没跑就先按需拉起来。
    // 这里不能像以前那样「服务器没跑就直接返回」——那样会把上一次留下的坏隧道
    // （指向一个已经没人监听的端口）一直挂在系统上，整机 DNS 全部超时，
    // 表现就是「所有域名都解析不了，直接访问 IP 却正常」。
    let Some(port) = ensure_local_dns_running(&app, state.inner()).await else {
        log::warn!(
            "本地 DNS 服务器不可用（{}），不接管系统 DNS",
            local_dns_failure_reason(state.inner())
        );
        #[cfg(target_os = "android")]
        if crate::mobile::is_tunnel_active() {
            log::warn!("收起已失效的 VPN 隧道");
            if let Err(e) = crate::mobile::stop_tunnel() {
                log::warn!("收起失效隧道失败: {}", e);
            }
        }
        return Ok(false);
    };

    #[cfg(target_os = "android")]
    {
        crate::mobile::auto_start_tunnel(port).await;
        Ok(crate::mobile::is_tunnel_active())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = port;
        Ok(false)
    }
}

/// 启动时自动运行内置 DNS 服务器（失败仅记录日志，不影响应用启动）
///
/// 只有移动端用它：内置 DNS 服务器 + VPN 隧道是移动端在不 root 的前提下让 hosts
/// 映射生效的唯一途径，所以无条件启动。桌面端直接改写系统 hosts，不需要它。
/// 两端都不再受任何开关控制——配置界面已从所有端移除，遗留设置不该让服务静默失效。
///
/// 注：这里只负责拉起本地 DNS 服务器；是否接管系统 DNS（VPN 隧道）取决于有没有
/// 配置处于启用状态，见下方以及 [`sync_mobile_tunnel`]。
pub async fn auto_start_dns_proxy(app: tauri::AppHandle) {
    use tauri::Manager;

    if !crate::hosts_path::is_mobile() {
        return;
    }

    let settings = crate::store::load_settings_public(&app);

    let state = app.state::<AppState>();
    let mapping_count = refresh_proxy_mappings(&state);
    // 没有任何启用的配置时不去接管系统 DNS（用户可能把开关都关掉了）
    let should_take_over = has_enabled_profile(state.inner());

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
        Ok(addr) => {
            log::info!("DNS 代理已自动启动: {}（映射 {} 条）", addr, mapping_count);
            // Android：本地 DNS 服务器就绪后接管系统 DNS。系统按包名永久记住 VPN
            // 授权，因此这里始终是静默的；用户从未授权过则弹出一次授权弹窗，
            // 曾拒绝过则不打扰（用户主动打开配置开关时会重新申请，见 toggle_profile）。
            //
            // 隧道转发端口必须用**实际绑定**的地址：首选端口被占用时服务器会退到
            // 系统随机端口，这里若还用配置里的端口，就会建出一条指向空端口的隧道，
            // 全机域名解析全部超时。
            #[cfg(target_os = "android")]
            if should_take_over {
                let tunnel_port = addr.port();
                tauri::async_runtime::spawn(async move {
                    crate::mobile::auto_start_tunnel(tunnel_port).await;
                });
            }
            #[cfg(not(target_os = "android"))]
            let _ = should_take_over;
        }
        // 失败原因已由 proxy 记录（见 last_error），诊断报告会展示
        Err(_) => {}
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

// ============ 诊断 ============

/// 诊断用：向本机 DNS 代理发一条真实查询，返回一行可读结论
///
/// 走的是 `127.0.0.1` 的普通 UDP，不经过 VPN 隧道，因此它只回答一个问题：
/// **本地 DNS 服务器本身是否正常工作**。配合隧道日志，就能把
/// 「隧道根本没把查询送进来」和「送进来了但本地服务器答不出来」彻底分开——
/// 这两种情况在真机上的表现完全一样（域名解析失败），只有这样才能区分。
async fn probe_local_dns(
    port: u16,
    name: &str,
    record_type: hickory_proto::rr::RecordType,
) -> String {
    use hickory_proto::op::{Message, MessageType, OpCode, Query};
    use hickory_proto::rr::{Name, RData};
    use std::time::{Duration, Instant};
    use tokio::net::UdpSocket;

    let Ok(target) = Name::from_ascii(name) else {
        return format!("域名非法（{name}）");
    };

    let mut query = Message::new();
    query
        .set_id(0x5E70)
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true);
    query.add_query(Query::query(target, record_type));

    let Ok(payload) = query.to_vec() else {
        return "构造查询报文失败".to_string();
    };

    let socket = match UdpSocket::bind("127.0.0.1:0").await {
        Ok(socket) => socket,
        Err(e) => return format!("创建本机 UDP 套接字失败（{e}）"),
    };
    if let Err(e) = socket.connect(("127.0.0.1", port)).await {
        return format!("连接 127.0.0.1:{port} 失败（{e}）");
    }

    let started = Instant::now();
    if let Err(e) = socket.send(&payload).await {
        return format!("发送查询失败（{e}）");
    }

    let mut buffer = vec![0u8; 4096];
    let received = match tokio::time::timeout(Duration::from_secs(2), socket.recv(&mut buffer)).await
    {
        Ok(Ok(size)) => size,
        Ok(Err(e)) => return format!("接收应答失败（{e}）"),
        Err(_) => {
            return format!("2 秒内无应答 —— 本地 DNS 服务器没有在 127.0.0.1:{port} 上正常服务");
        }
    };
    let elapsed = started.elapsed().as_millis();

    let Ok(response) = Message::from_vec(&buffer[..received]) else {
        return format!("应答报文无法解析（{received} 字节）");
    };

    let rcode = response.response_code();
    let answers: Vec<String> = response
        .answers()
        .iter()
        .map(|record| match record.data() {
            RData::A(a) => a.0.to_string(),
            RData::AAAA(a) => a.0.to_string(),
            other => format!("{other:?}"),
        })
        .collect();

    if answers.is_empty() {
        format!("无记录（{rcode:?}，0 条答案，{elapsed}ms）")
    } else {
        format!("{}（{rcode:?}，{elapsed}ms）", answers.join(", "))
    }
}

/// 生成诊断报告：本地 DNS 服务器 + 映射内容 + 本机自测 + 原生隧道日志
///
/// 移动端「域名打不开、但直接访问 IP 正常」这类问题，得把「隧道是否接到查询」
/// 「本地服务器是否应答」「映射表里到底有什么」「上游是否可用」四条链路一次摊开
/// 才定位得了，所以这里一次性全给出来，用户复制粘贴即可，不必装 adb。
#[tauri::command]
pub async fn get_diagnostics(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use hickory_proto::rr::RecordType;

    let status = state.proxy.status();
    let entries = state.proxy.mapping_entries();
    let settings = crate::store::load_settings_public(&app);
    let last_error = state.proxy.last_error();
    // 原生隧道报告先取出来：里面带着「隧道实际把 DNS 转发到哪个端口」，
    // 要和本地服务器真正监听的端口对照——两者不一致就是整机 DNS 全部超时，
    // 这是「所有应用都上不了网、直接访问 IP 却正常」的头号原因，必须一眼可见。
    let native = crate::mobile::diagnostics_report();
    // 端口 0 = 隧道从未建立过（原生侧变量初值），不能当成「转发到 0 端口」来解读
    let tunnel_port = native
        .lines()
        .find_map(|line| line.trim().strip_prefix("转发目标: 127.0.0.1:"))
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port != 0);
    let listen_port = status
        .listen_addr
        .as_deref()
        .and_then(|addr| addr.rsplit(':').next())
        .and_then(|value| value.parse::<u16>().ok());
    let mut report = String::new();

    report.push_str("==== Set Hosts 诊断报告 ====\n");
    report.push_str(&format!(
        "时间: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    report.push_str(&format!(
        "平台: {} / {}\n",
        std::env::consts::OS,
        if crate::hosts_path::is_mobile() {
            "移动端"
        } else {
            "桌面端"
        }
    ));

    report.push_str("\n---- 本地 DNS 服务器 ----\n");
    report.push_str(&format!("运行中: {}\n", status.running));
    report.push_str(&format!(
        "监听: {}\n",
        status.listen_addr.as_deref().unwrap_or("（未监听）")
    ));
    report.push_str(&format!(
        "首选端口: {}（0 = 用默认 {}；被占用时会自动改用系统随机端口）\n",
        settings.dns_proxy_port,
        dns_proxy::DEFAULT_PORT
    ));
    // 这些域名被开关来回改写，应答会被压到 1 秒 TTL——否则客户端缓存住上游应答，
    // 重新打开配置时不再发查询，就成了「关了再开打不开、要重启应用」
    report.push_str(&format!(
        "管理域名: {} 个（含未启用；应答 TTL 压到 1 秒，保证开关立即生效）\n",
        state.proxy.managed_count()
    ));
    // 服务器起不来是移动端「映射完全没生效」的头号原因，失败原因必须直接摊在报告里
    if !status.running {
        match &last_error {
            Some(reason) => report.push_str(&format!("上次启动失败: {reason}\n")),
            None => report.push_str("上次启动失败: （尚无失败记录，可能压根没尝试启动过）\n"),
        }
    }
    report.push_str(&format!(
        "上游: {}\n",
        if status.upstream.is_empty() {
            "（未配置）".to_string()
        } else {
            status.upstream.join(", ")
        }
    ));
    report.push_str(&format!(
        "统计: 映射 {} 条，命中 {} 次，转发上游 {} 次\n",
        status.mapping_count, status.hit_count, status.forward_count
    ));

    // 端口一致性：这是最重要的一行结论
    match (listen_port, tunnel_port) {
        (Some(listen), Some(tunnel)) if listen == tunnel => {
            report.push_str(&format!("隧道转发端口: {}（与监听端口一致 ✓）\n", tunnel));
        }
        (Some(listen), Some(tunnel)) => {
            report.push_str(&format!(
                "隧道转发端口: {} ✗ 与监听端口 {} 不一致 —— 整机 DNS 都会超时，\
                 所有应用都上不了网、只有直接访问 IP 才行\n",
                tunnel, listen
            ));
        }
        (None, Some(tunnel)) => {
            report.push_str(&format!(
                "隧道转发端口: {} ✗ 但本地 DNS 服务器没有在监听 —— 整机 DNS 都会超时\n",
                tunnel
            ));
        }
        (_, None) => report.push_str("隧道转发端口: 隧道未建立（或不在此设备上）\n"),
    }

    if entries.is_empty() {
        report.push_str("映射内容: 空 —— 没有任何启用中的配置，或配置里没有可用条目\n");
    } else {
        report.push_str(&format!(
            "映射内容（共 {} 条，最多列 30 条）:\n",
            entries.len()
        ));
        for (domain, ip) in entries.iter().take(30) {
            report.push_str(&format!("  {domain} -> {ip}\n"));
        }
    }

    report.push_str("\n---- 自测：从本机直连本地 DNS 服务器 ----\n");
    match status.listen_addr.as_deref() {
        Some(addr) => {
            let port = addr
                .rsplit(':')
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(dns_proxy::DEFAULT_PORT);
            if let Some((domain, ip)) = entries.first() {
                let record = if ip.is_ipv6() {
                    RecordType::AAAA
                } else {
                    RecordType::A
                };
                report.push_str(&format!(
                    "已映射 {domain}（期望 {ip}）: {}\n",
                    probe_local_dns(port, domain, record).await
                ));
            }
            report.push_str(&format!(
                "未映射 example.com（应转发上游）: {}\n",
                probe_local_dns(port, "example.com", RecordType::A).await
            ));
        }
        None => report.push_str("本地 DNS 服务器未在运行，跳过自测\n"),
    }

    report.push_str("\n==== VPN 隧道（原生） ====\n");
    if native.trim().is_empty() {
        report.push_str("（没有原生隧道：桌面端不使用隧道，或原生桥接尚未就绪）\n");
    } else {
        report.push_str(&native);
    }

    Ok(report)
}

/// 清空诊断日志（目前只有原生隧道日志需要清）
#[tauri::command]
pub fn clear_diagnostics() {
    crate::mobile::clear_diagnostics();
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
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
        return Ok(());
    }

    // 移动端没有可打开的文件管理器入口
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = parent;
        Err("当前平台不支持打开文件夹".to_string())
    }
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

// ============ 退出应用 ============

/// 退出应用（移动端「再按一次退出」确认后由前端调用）。
///
/// Android：通知 MainActivity 结束 Activity 并终止进程（会先停掉 VPN 隧道），
/// 前端调用前已把未保存的编辑内容落盘；桌面端当前没有入口调用它，仅作兜底。
#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    #[cfg(target_os = "android")]
    {
        if let Err(e) = crate::mobile::android::exit_app() {
            log::error!("调用原生退出失败，退回 Tauri 退出: {e}");
            app.exit(0);
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        app.exit(0);
    }
}

//! 移动端原生桥接：VPN 隧道把系统 DNS 指向应用内的本地 DNS 服务器
//!
//! 本地 DNS 服务器已由 [`crate::dns_proxy`] 实现；移动端要让映射真正生效，
//! 还需把系统 DNS 流量接管到该服务器：
//!
//! - Android：由 `gen/android` 原生工程中的 `DnsVpnService`（VpnService 隧道）
//!   把系统解析器的查询接进来，再转交给 `127.0.0.1:<port>`，详见 [`android`]；
//! - iOS：[`ios::start_tunnel`] 需 Apple 付费开发者账号与 Network Extension entitlement。
//!
//! 未接入原生隧道时，[`is_tunnel_active`] 返回 false，映射不会在系统范围内生效。

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
#[allow(dead_code)]
pub mod ios;

/// 供原生层回调（VPN 授权结果）时向前端广播事件使用的 AppHandle。
///
/// Android 侧 `DnsVpn.onActivityResult` 拿到授权结果后会回调 Rust，而那时已经
/// 脱离 Tauri 的命令上下文，拿不到 `app`，因此在这里留一份。
#[cfg(target_os = "android")]
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// 记录 AppHandle（应用启动时调用一次）
#[cfg(target_os = "android")]
pub fn init_app_handle(handle: tauri::AppHandle) {
    if APP_HANDLE.set(handle).is_err() {
        log::warn!("AppHandle 已初始化，忽略重复设置");
    }
}

/// 系统 VPN 授权结果回调 → 广播 `vpn-consent` 事件。
///
/// 移动端把配置开关从「全部关闭」切到「打开」时会申请系统 VPN 授权，授权结果是
/// 异步回传的；前端据此在用户拒绝时把刚打开的开关回滚（见 `stores/profiles.ts`）。
#[cfg(target_os = "android")]
pub fn notify_consent_result(granted: bool) {
    use tauri::Emitter;

    match APP_HANDLE.get() {
        Some(handle) => {
            if let Err(e) = handle.emit("vpn-consent", serde_json::json!({ "granted": granted })) {
                log::warn!("广播 vpn-consent 事件失败: {e}");
            }
        }
        None => log::warn!("AppHandle 尚未就绪，无法广播 vpn-consent 事件"),
    }
}

/// 启动 VPN 隧道，把系统 DNS 指向本地 DNS 服务器端口。
///
/// 未授权时**总会**发起一次系统 VPN 授权请求（即使用户之前拒绝过），用于响应用户
/// 主动打开配置开关的动作；自动接管请用 [`auto_start_tunnel`]。
pub fn start_tunnel(port: u16) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::start_vpn(port)
    }
    #[cfg(target_os = "ios")]
    {
        ios::start_tunnel(&format!("127.0.0.1:{}", port))
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = port;
        Err("当前平台无需 VPN 隧道".to_string())
    }
}

/// 停止 VPN 隧道
pub fn stop_tunnel() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::stop_vpn()
    }
    #[cfg(target_os = "ios")]
    {
        ios::stop_tunnel()
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Err("当前平台无需 VPN 隧道".to_string())
    }
}

/// Android：等原生桥接（Activity）就绪后自动建立 VPN 隧道。
///
/// 系统对 VPN 授权是按包名**永久记忆**的：第一次调用会弹出一次系统授权弹窗，
/// 之后每次启动应用都能静默接管系统 DNS，用户无需再做任何设置；用户曾拒绝过
/// 授权则不再打扰（只有用户主动打开配置开关才会重新申请）。
/// Activity 创建完成前 JNI 调用必然失败，因此这里按 250ms 轮询等待（最多约 15 秒）。
#[cfg(target_os = "android")]
pub async fn auto_start_tunnel(port: u16) {
    for _ in 0..60 {
        if android::is_ready() {
            if android::is_vpn_active() {
                log::info!("VPN 隧道已接管系统 DNS，无需重复建立");
                return;
            }
            match android::auto_start_vpn(port) {
                Ok(()) => log::info!("VPN 隧道已自动建立：系统 DNS 指向 127.0.0.1:{port}"),
                Err(e) => log::warn!("自动建立 VPN 隧道失败: {e}"),
            }
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    log::warn!("自动建立 VPN 隧道超时：原生桥接未就绪");
}

/// 读取原生隧道诊断报告（桌面端恒为空串：不使用隧道）
pub fn diagnostics_report() -> String {
    #[cfg(target_os = "android")]
    {
        match android::diagnostics_dump() {
            Ok(text) => text,
            Err(err) => format!("读取原生诊断日志失败：{err}"),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        String::new()
    }
}

/// 清空原生隧道诊断日志（桌面端空操作）
pub fn clear_diagnostics() {
    #[cfg(target_os = "android")]
    {
        if let Err(err) = android::diagnostics_clear() {
            log::warn!("清空原生诊断日志失败: {err}");
        }
    }
}

/// VPN 隧道是否已接管系统 DNS
pub fn is_tunnel_active() -> bool {
    #[cfg(target_os = "android")]
    {
        android::is_vpn_active()
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

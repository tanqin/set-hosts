//! 跨平台 hosts 文件路径解析

use std::path::PathBuf;

/// 返回系统 hosts 文件路径
/// 移动端返回 None（由 DNS 代理处理，不直接读写系统 hosts）
pub fn hosts_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // %SystemRoot%\System32\drivers\etc\hosts
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        Some(PathBuf::from(system_root)
            .join("System32")
            .join("drivers")
            .join("etc")
            .join("hosts"))
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        Some(PathBuf::from("/etc/hosts"))
    }

    #[cfg(target_os = "android")]
    {
        // Android 系统 hosts 需要 root，这里返回 None 由 DNS 代理处理
        None
    }

    #[cfg(target_os = "ios")]
    {
        // iOS 沙盒限制，无法访问系统 hosts，由 DNS 代理处理
        None
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
        target_os = "ios"
    )))]
    {
        None
    }
}

/// 是否为桌面平台（直接读写系统 hosts）
pub fn is_desktop() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux", target_os = "macos"))
}

/// 是否为移动平台（靠内置 DNS 服务器 + VPN 隧道让映射生效）
pub fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

//! 跨平台 DNS 缓存刷新

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use std::process::Command;

/// 刷新系统 DNS 缓存，返回命令输出摘要
pub fn flush_dns_cache() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        flush_windows()
    }
    #[cfg(target_os = "macos")]
    {
        // 应用本身不是 root：普通用户执行 dscacheutil / killall 常被拒绝。
        // Windows 端进程已提权、刷新必然生效，这里失败时再走一次 osascript 提权，
        // 保证"应用配置后 DNS 缓存真的被清掉"，不至于静默失效。
        flush_macos().or_else(|reason| flush_macos_elevated(reason))
    }
    #[cfg(target_os = "linux")]
    {
        // 同上：resolvectl / nscd 常需要 polkit 授权，失败后用 pkexec 重试
        flush_linux().or_else(|reason| flush_linux_elevated(reason))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("当前平台不支持 DNS 缓存刷新".to_string())
    }
}

#[cfg(target_os = "windows")]
fn flush_windows() -> Result<String, String> {
    // 必须隐藏控制台窗口：GUI 程序 spawn cmd/ipconfig 会闪黑框，
    // 而每次应用/切换 profile 都会走到这里
    let output = crate::process::hidden(Command::new("cmd"))
        .args(["/c", "ipconfig", "/flushdns"])
        .output()
        .map_err(|e| format!("执行 ipconfig 失败: {}", e))?;
    if output.status.success() {
        Ok("DNS 缓存已刷新".to_string())
    } else {
        // stderr 可能是 GBK 编码，用 GBK 解码
        let stderr = decode_windows_output(&output.stderr);
        Err(format!("ipconfig /flushdns 失败: {}", stderr))
    }
}

/// Windows 命令输出默认为系统编码（中文为 GBK/CP936），需转 UTF-8
#[cfg(target_os = "windows")]
fn decode_windows_output(bytes: &[u8]) -> String {
    // 尝试 UTF-8，失败则用 GBK 解码
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            // GBK → UTF-8
            let (cow, _, _) = encoding_rs::GBK.decode(bytes);
            cow.into_owned()
        }
    }
}

#[cfg(target_os = "macos")]
fn flush_macos() -> Result<String, String> {
    // macOS 的 DNS 缓存分散在两处：目录服务缓存（dscacheutil）与 mDNSResponder。
    // 与 Windows / Linux 对齐：两步都失败才返回 Err，避免"刷新其实没生效"却被当成成功。
    let mut msg = String::new();
    let mut any_ok = false;

    if which("dscacheutil").is_ok() {
        match Command::new("dscacheutil").args(["-flushcache"]).output() {
            Ok(o) if o.status.success() => {
                msg.push_str("dscacheutil -flushcache: 成功\n");
                any_ok = true;
            }
            Ok(o) => msg.push_str(&format!(
                "dscacheutil: {}\n",
                String::from_utf8_lossy(&o.stderr).trim()
            )),
            Err(e) => msg.push_str(&format!("dscacheutil 启动失败: {}\n", e)),
        }
    } else {
        msg.push_str("dscacheutil: 不存在\n");
    }

    if which("killall").is_ok() {
        match Command::new("killall")
            .args(["-HUP", "mDNSResponder"])
            .status()
        {
            Ok(s) if s.success() => {
                msg.push_str("killall -HUP mDNSResponder: 成功");
                any_ok = true;
            }
            Ok(_) => {
                msg.push_str("killall -HUP mDNSResponder: 返回非 0（通常是因为未以管理员运行）")
            }
            Err(e) => msg.push_str(&format!("killall 启动失败: {}", e)),
        }
    } else {
        msg.push_str("killall: 不存在");
    }

    if any_ok {
        Ok(msg)
    } else {
        Err(format!(
            "macOS DNS 缓存刷新失败: {}（可尝试在终端执行：sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder）",
            msg
        ))
    }
}

/// 提权重试：普通权限刷新失败时用 pkexec 再刷一次
///
/// polkit 的授权在数分钟内会复用（auth_admin_keep），通常紧随写入的提权之后执行，
/// 不会额外弹密码框；没有 pkexec 或无授权时返回原始失败原因，不掩盖信息。
#[cfg(target_os = "linux")]
fn flush_linux_elevated(reason: String) -> Result<String, String> {
    if which("pkexec").is_err() {
        return Err(reason);
    }
    let attempts: &[&[&str]] = &[
        &["resolvectl", "flush-caches"],
        &["systemd-resolve", "--flush-caches"],
        &["nscd", "-i", "hosts"],
    ];
    for cmd in attempts {
        if which(cmd[0]).is_err() {
            continue;
        }
        if let Ok(status) = Command::new("pkexec").args(*cmd).status() {
            if status.success() {
                return Ok(format!("以管理员权限执行 {} {:?}: 成功", cmd[0], &cmd[1..]));
            }
        }
    }
    Err(reason)
}

#[cfg(target_os = "linux")]
fn flush_linux() -> Result<String, String> {
    // 依次尝试，首个成功即停
    let attempts: &[&[&str]] = &[
        &["resolvectl", "flush-caches"],
        &["systemd-resolve", "--flush-caches"],
        &["nscd", "-i", "hosts"],
    ];
    let mut last_err = String::new();
    for cmd in attempts {
        if which(cmd[0]).is_err() {
            last_err = format!("{} 不存在", cmd[0]);
            continue;
        }
        let output = match Command::new(cmd[0]).args(&cmd[1..]).output() {
            Ok(o) => o,
            Err(e) => {
                last_err = format!("启动 {} 失败: {}", cmd[0], e);
                continue;
            }
        };
        if output.status.success() {
            return Ok(format!("{} {:?}: 成功", cmd[0], &cmd[1..]));
        }
        last_err = format!(
            "{} 失败: {}",
            cmd[0],
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Err(format!("未检测到可用的 DNS 缓存服务: {}", last_err))
}

/// 提权重试：普通权限刷新失败时用 osascript 再刷一次（macOS 与 Linux 同理）
#[cfg(target_os = "macos")]
fn flush_macos_elevated(reason: String) -> Result<String, String> {
    if which("osascript").is_err() {
        return Err(reason);
    }
    let script = "do shell script \"dscacheutil -flushcache; killall -HUP mDNSResponder\" with administrator privileges";
    let output = match Command::new("osascript").args(["-e", script]).output() {
        Ok(o) => o,
        Err(_) => return Err(reason),
    };
    if output.status.success() {
        Ok("以管理员权限刷新 DNS 缓存成功".to_string())
    } else {
        // 用户取消或授权失败：回传原始原因，保留首次尝试的失败细节
        Err(reason)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn which(cmd: &str) -> Result<(), ()> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let full = std::path::Path::new(dir).join(cmd);
            if full.exists() {
                return Ok(());
            }
        }
    }
    Err(())
}

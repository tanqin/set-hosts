//! 跨平台 DNS 缓存刷新

use std::process::Command;

/// 刷新系统 DNS 缓存，返回命令输出摘要
pub fn flush_dns_cache() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        flush_windows()
    }
    #[cfg(target_os = "macos")]
    {
        flush_macos()
    }
    #[cfg(target_os = "linux")]
    {
        flush_linux()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("当前平台不支持 DNS 缓存刷新".to_string())
    }
}

#[cfg(target_os = "windows")]
fn flush_windows() -> Result<String, String> {
    let output = Command::new("cmd")
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
    // 先刷 dscacheutil，再 HUP mDNSResponder（忽略后者失败）
    let mut msg = String::new();
    let out1 = Command::new("dscacheutil")
        .args(["-flushcache"])
        .output();
    match out1 {
        Ok(o) if o.status.success() => msg.push_str("dscacheutil: 成功\n"),
        Ok(o) => msg.push_str(&format!("dscacheutil: {}\n", String::from_utf8_lossy(&o.stderr))),
        Err(e) => msg.push_str(&format!("dscacheutil 启动失败: {}\n", e)),
    }
    let _ = Command::new("killall")
        .args(["-HUP", "mDNSResponder"])
        .status();
    msg.push_str("killall -HUP mDNSResponder: 已发送");
    Ok(msg)
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

#[cfg(target_os = "linux")]
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

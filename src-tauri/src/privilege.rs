//! 权限提升：写入系统 hosts 文件
//!
//! 桌面端策略：
//! - 若进程本身已有足够权限（管理员/sudo），直接写入
//! - 否则通过原生 GUI 提升工具（UAC/pkexec/osascript）复制覆盖

use std::path::PathBuf;

/// 获取 hosts 文件路径（桌面端必有）
fn hosts_path() -> Result<PathBuf, String> {
    crate::hosts_path::hosts_path().ok_or_else(|| "当前平台不支持直接修改系统 hosts".to_string())
}

/// 将内容写入系统 hosts
/// 若当前进程已有管理员权限，直接写入；否则走提权流程
pub fn write_hosts_elevated(content: &str) -> Result<(), String> {
    let hosts = hosts_path()?;

    // 尝试直接写入
    match std::fs::write(&hosts, content) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // 权限不足，走提权流程
        }
        Err(e) => return Err(format!("写入 hosts 失败: {}", e)),
    }

    // 提权写入：写临时文件 → 调用原生 GUI 工具复制覆盖
    let temp = write_temp_file(content)?;
    copy_with_elevation(&temp, &hosts)
}

/// 写临时文件，返回路径
fn write_temp_file(content: &str) -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let name = format!("hosts.new.{}", uuid::Uuid::new_v4());
    let path = temp_dir.join(name);
    std::fs::write(&path, content).map_err(|e| format!("写临时文件失败: {}", e))?;
    Ok(path)
}

// ============ Windows ============

#[cfg(target_os = "windows")]
fn copy_with_elevation(temp: &PathBuf, hosts: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    let temp_str = temp.to_string_lossy().replace('\'', "''");
    let hosts_str = hosts.to_string_lossy().replace('\'', "''");
    let ps_cmd = format!(
        "Start-Process cmd -ArgumentList '/c','copy /Y \"{}\" \"{}\"' -Verb RunAs -WindowStyle Hidden -Wait",
        temp_str, hosts_str
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output()
        .map_err(|e| format!("启动 PowerShell 失败: {}", e))?;

    let _ = std::fs::remove_file(temp);

    if !output.status.success() {
        let err = decode_windows_bytes(&output.stderr);
        return Err(format!("UAC 提升失败或被用户取消: {}", err));
    }
    Ok(())
}

/// Windows 命令输出默认为系统编码（中文为 GBK），需转 UTF-8
#[cfg(target_os = "windows")]
fn decode_windows_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::GBK.decode(bytes);
            cow.into_owned()
        }
    }
}

// ============ macOS ============

#[cfg(target_os = "macos")]
fn copy_with_elevation(temp: &PathBuf, hosts: &PathBuf) -> Result<(), String> {
    use std::process::Command;
    let temp_str = temp.to_string_lossy().to_string();
    let hosts_str = hosts.to_string_lossy().to_string();
    let script = format!(
        "do shell script \"cp '{}' '{}'\" with administrator privileges",
        temp_str.replace('\'', "'\\''"),
        hosts_str.replace('\'', "'\\''")
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("启动 osascript 失败: {}", e))?;
    let _ = std::fs::remove_file(temp);
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("macOS 权限提升失败: {}", err));
    }
    Ok(())
}

// ============ Linux ============

#[cfg(target_os = "linux")]
fn copy_with_elevation(temp: &PathBuf, hosts: &PathBuf) -> Result<(), String> {
    use std::process::Command;
    let temp_str = temp.to_string_lossy().to_string();
    let hosts_str = hosts.to_string_lossy().to_string();

    if which("pkexec").is_ok() {
        let status = Command::new("pkexec")
            .args(["cp", "-f", &temp_str, &hosts_str])
            .status()
            .map_err(|e| format!("启动 pkexec 失败: {}", e))?;
        let _ = std::fs::remove_file(temp);
        if !status.success() {
            return Err("pkexec 权限提升被取消或失败".to_string());
        }
        return Ok(());
    }

    let _ = std::fs::remove_file(temp);
    Err("未检测到 pkexec，请安装 polkit (sudo apt install policykit-1) 或手动用 sudo 应用".to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn copy_with_elevation(_temp: &PathBuf, _hosts: &PathBuf) -> Result<(), String> {
    Err("当前平台不支持权限提升写入 hosts".to_string())
}

/// 检测命令是否存在（Linux）
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

/// 读取当前系统 hosts 内容
pub fn read_hosts_content() -> Result<String, String> {
    let path = hosts_path()?;
    std::fs::read_to_string(&path).map_err(|e| format!("读取 hosts 失败: {}", e))
}

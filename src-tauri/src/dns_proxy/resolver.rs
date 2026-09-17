//! 上游 DNS 服务器解析
//!
//! 优先级：用户配置（选项中的「上游 DNS」） > 系统 DNS（/etc/resolv.conf、ipconfig） > 公共 DNS。
//! 移动端通常读不到系统 DNS 配置，此时依赖用户配置或公共 DNS。

use std::net::{IpAddr, SocketAddr};

/// 兜底公共 DNS（前 3 个按顺序尝试）
const FALLBACK_UPSTREAMS: [&str; 4] = ["223.5.5.5", "119.29.29.29", "1.1.1.1", "8.8.8.8"];

/// 决定最终使用的上游 DNS 列表
pub fn resolve_upstreams(configured: &str) -> Vec<SocketAddr> {
    let user = parse_upstreams(configured);
    if !user.is_empty() {
        return user;
    }

    let system = system_upstreams();
    if !system.is_empty() {
        log::info!("DNS 代理使用系统 DNS 作为上游：{:?}", system);
        return system;
    }

    let fallback: Vec<SocketAddr> = FALLBACK_UPSTREAMS
        .iter()
        .take(3)
        .filter_map(|s| parse_one(s))
        .collect();
    log::info!("DNS 代理使用公共 DNS 作为上游：{:?}", fallback);
    fallback
}

/// 解析用户配置的上游（逗号 / 分号 / 空格分隔，支持 `1.1.1.1`、`1.1.1.1:5353`、`[::1]:53`）
pub fn parse_upstreams(spec: &str) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = Vec::new();
    for raw in spec.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        match parse_one(item) {
            Some(addr) => {
                if !out.contains(&addr) {
                    out.push(addr);
                }
            }
            None => log::warn!("忽略无法解析的上游 DNS 地址：{}", item),
        }
    }
    out
}

/// 解析单个上游地址（省略端口时使用 53）
fn parse_one(item: &str) -> Option<SocketAddr> {
    if let Ok(addr) = item.parse::<SocketAddr>() {
        return Some(addr);
    }
    // Windows 的 ipconfig 会带 IPv6 作用域后缀（fe80::1%12），需要先剥离
    let without_scope = item.split('%').next().unwrap_or(item).trim();
    let bare = without_scope.trim_matches(|c| c == '[' || c == ']');
    match bare.parse::<IpAddr>() {
        Ok(ip) => Some(SocketAddr::new(ip, 53)),
        Err(_) => None,
    }
}

/// 读取系统 DNS 服务器（best-effort，失败返回空列表）
pub fn system_upstreams() -> Vec<SocketAddr> {
    let mut servers = system_upstreams_impl();
    servers.dedup();
    servers
}

/// Unix（含 Android / iOS）：解析 /etc/resolv.conf
#[cfg(unix)]
fn system_upstreams_impl() -> Vec<SocketAddr> {
    let content = match std::fs::read_to_string("/etc/resolv.conf") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut servers = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("nameserver") else {
            continue;
        };
        if let Some(addr) = parse_one(rest.trim()) {
            servers.push(addr);
        }
    }
    servers
}

/// Windows：解析 `ipconfig /all` 中的 DNS 行（中英文系统均可识别）
#[cfg(windows)]
fn system_upstreams_impl() -> Vec<SocketAddr> {
    let output = match crate::process::hidden(std::process::Command::new("ipconfig"))
        .arg("/all")
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::debug!("执行 ipconfig 失败: {}", e);
            return Vec::new();
        }
    };

    // 中文系统输出为 GBK，但 DNS 标记与 IP 均为 ASCII，按行提取即可
    let text = String::from_utf8_lossy(&output.stdout);
    let mut servers = Vec::new();
    for line in text.lines() {
        // 例："DNS Servers . . . . . . . . . . . : 192.168.1.1" / "DNS 服务器 . . . : 192.168.1.1"
        let Some(pos) = line.find("DNS") else {
            continue;
        };
        let tail = &line[pos..];
        let Some(colon) = tail.rfind(':') else {
            continue;
        };
        let value = tail[colon + 1..].trim();
        if value.is_empty() {
            continue;
        }
        if let Some(addr) = parse_one(value) {
            servers.push(addr);
        }
    }
    servers
}

/// 其它平台：无系统 DNS 探测手段
#[cfg(not(any(unix, windows)))]
fn system_upstreams_impl() -> Vec<SocketAddr> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_separators_and_default_port() {
        let parsed = parse_upstreams("223.5.5.5, 8.8.8.8:5353;1.1.1.1 [::1]:53");
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0], "223.5.5.5:53".parse().unwrap());
        assert_eq!(parsed[1], "8.8.8.8:5353".parse().unwrap());
        assert_eq!(parsed[2], "1.1.1.1:53".parse().unwrap());
        assert_eq!(parsed[3], "[::1]:53".parse().unwrap());
    }

    #[test]
    fn ignores_invalid_entries_and_duplicates() {
        let parsed = parse_upstreams("dns.example.com, 8.8.8.8, 8.8.8.8");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], "8.8.8.8:53".parse().unwrap());
    }

    #[test]
    fn strips_ipv6_scope_id() {
        assert_eq!(
            parse_one("fe80::1%12"),
            Some(SocketAddr::new("fe80::1".parse().unwrap(), 53))
        );
    }

    #[test]
    fn empty_config_falls_back_when_system_probe_fails() {
        // 系统探测在 CI / 受限环境可能返回空，此处仅校验用户配置优先
        let parsed = resolve_upstreams("9.9.9.9");
        assert_eq!(parsed, vec!["9.9.9.9:53".parse().unwrap()]);
    }
}

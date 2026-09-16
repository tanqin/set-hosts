//! hosts 文本 ↔ 结构化条目互转
//!
//! 为避免覆盖系统原有的 hosts 条目（如 127.0.0.1 localhost），
//! 我们用一对标记块包裹「由 Set Hosts 管理」的条目，应用时只替换该块：
//!
//! ```
//! # >>> Set Hosts Managed >>>
//! 192.168.1.1 dev.local
//! # <<< Set Hosts Managed <<<
//! ```

use crate::models::HostEntry;

/// 托管块起始标记
pub const MANAGED_START: &str = "# >>> Set Hosts Managed >>>";
/// 托管块结束标记
pub const MANAGED_END: &str = "# <<< Set Hosts Managed <<<";

/// 解析 hosts 文本为条目列表
/// 格式：每行 `IP  domain1 [domain2 ...] [# comment]`
/// 纯注释行（# 开头）保留为 comment-only 条目
pub fn parse_hosts_text(text: &str) -> Vec<HostEntry> {
    let mut entries = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        // 跳过空行
        if trimmed.is_empty() {
            continue;
        }

        // 纯注释行：保留为 comment-only 条目
        if trimmed.starts_with('#') {
            entries.push(HostEntry {
                id: uuid::Uuid::new_v4().to_string(),
                ip: String::new(),
                domains: Vec::new(),
                enabled: true,
                comment: Some(trimmed[1..].trim().to_string()),
            });
            continue;
        }

        // 提取行尾注释
        let (main_part, comment) = match trimmed.find('#') {
            Some(pos) => {
                let c = trimmed[pos + 1..].trim().to_string();
                (trimmed[..pos].trim().to_string(), if c.is_empty() { None } else { Some(c) })
            }
            None => (trimmed.to_string(), None),
        };

        // 按 whitespace 拆分
        let parts: Vec<&str> = main_part.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let ip = parts[0].to_string();
        let domains: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        if domains.is_empty() {
            continue;
        }

        entries.push(HostEntry {
            id: uuid::Uuid::new_v4().to_string(),
            ip,
            domains,
            enabled: true,
            comment,
        });
    }

    entries
}

/// 将条目列表渲染为 hosts 文本（仅条目本身，不含标记块）
pub fn render_hosts_text(entries: &[HostEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        if !entry.enabled {
            continue;
        }
        // 纯注释条目（无 IP 和域名）
        if entry.ip.is_empty() && entry.domains.is_empty() {
            if let Some(c) = &entry.comment {
                out.push_str(&format!("# {}\n", c));
            }
            continue;
        }
        // IP + 域名（空格分隔）
        let line = format!("{} {}", entry.ip, entry.domains.join(" "));
        if let Some(c) = &entry.comment {
            out.push_str(&format!("{} # {}\n", line, c));
        } else {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// 从 hosts 内容中移除托管块（保留其余所有内容）
///
/// 若不存在托管块，原样返回。
pub fn strip_managed_block(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|l| l.trim() == MANAGED_START);
    let end = lines.iter().position(|l| l.trim() == MANAGED_END);

    match (start, end) {
        (Some(s), Some(e)) if e > s => {
            // 移除 s..=e 行
            let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
            kept.extend_from_slice(&lines[..s]);
            if e + 1 < lines.len() {
                kept.extend_from_slice(&lines[e + 1..]);
            }
            let out = kept.join("\n");
            // 规范化末尾换行
            let trimmed = out.trim_end();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{}\n", trimmed)
            }
        }
        _ => content.to_string(),
    }
}

/// 将新的托管条目合并到 hosts 内容中
///
/// - 移除旧的托管块（若存在）
/// - 在文件末尾追加新的托管块（含新渲染的条目）
/// - 其余内容原样保留
pub fn merge_managed_block(original: &str, entries: &[HostEntry]) -> String {
    let stripped = strip_managed_block(original);
    let rendered = render_hosts_text(entries);

    let mut out = stripped.trim_end().to_string();
    if !out.is_empty() {
        out.push('\n');
        out.push('\n');
    }
    out.push_str(MANAGED_START);
    out.push('\n');
    if !rendered.is_empty() {
        out.push_str(rendered.trim_end());
        out.push('\n');
    }
    out.push_str(MANAGED_END);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let text = "127.0.0.1 localhost\n192.168.1.1 web.local api.local # 测试";
        let entries = parse_hosts_text(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].domains, vec!["web.local", "api.local"]);
        assert_eq!(entries[1].comment.as_deref(), Some("测试"));

        let rendered = render_hosts_text(&entries);
        assert!(rendered.contains("127.0.0.1 localhost"));
        assert!(rendered.contains("192.168.1.1 web.local api.local # 测试"));
    }

    #[test]
    fn merge_preserves_other_entries() {
        let original = "127.0.0.1 localhost\n::1 localhost\n";
        let entries = parse_hosts_text("192.168.1.1 dev.local");
        let merged = merge_managed_block(original, &entries);

        // 系统原有条目保留
        assert!(merged.contains("127.0.0.1 localhost"));
        assert!(merged.contains("::1 localhost"));
        // 托管块标记存在
        assert!(merged.contains(MANAGED_START));
        assert!(merged.contains(MANAGED_END));
        // 新条目在托管块内
        assert!(merged.contains("192.168.1.1 dev.local"));
    }

    #[test]
    fn merge_replaces_old_block() {
        let original = format!(
            "127.0.0.1 localhost\n{}\n10.0.0.1 old.local\n{}\n",
            MANAGED_START, MANAGED_END
        );
        let entries = parse_hosts_text("192.168.1.1 new.local");
        let merged = merge_managed_block(&original, &entries);

        assert!(merged.contains("127.0.0.1 localhost"));
        assert!(!merged.contains("10.0.0.1 old.local"));
        assert!(merged.contains("192.168.1.1 new.local"));
        // 只应有一对标记块
        assert_eq!(merged.matches(MANAGED_START).count(), 1);
        assert_eq!(merged.matches(MANAGED_END).count(), 1);
    }

    #[test]
    fn strip_managed_block_removes_only_block() {
        let original = format!(
            "127.0.0.1 localhost\n{}\n10.0.0.1 old.local\n{}\n::1 localhost\n",
            MANAGED_START, MANAGED_END
        );
        let stripped = strip_managed_block(&original);
        assert!(stripped.contains("127.0.0.1 localhost"));
        assert!(stripped.contains("::1 localhost"));
        assert!(!stripped.contains("10.0.0.1 old.local"));
        assert!(!stripped.contains(MANAGED_START));
    }
}

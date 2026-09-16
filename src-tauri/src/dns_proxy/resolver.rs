//! 上游 DNS 转发（未命中时用系统 resolver 解析）

use hickory_resolver::TokioAsyncResolver;
use std::net::IpAddr;

/// 用系统配置解析域名
pub async fn lookup(name: &str) -> Result<Vec<IpAddr>, String> {
    let resolver = TokioAsyncResolver::tokio_from_system_conf()
        .map_err(|e| format!("创建 resolver 失败: {}", e))?;
    let lookup = resolver
        .lookup_ip(name)
        .await
        .map_err(|e| format!("解析 {} 失败: {}", name, e))?;
    Ok(lookup.iter().collect())
}

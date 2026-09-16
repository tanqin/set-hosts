//! 本地 DNS 服务器：tokio UDP + hickory-proto 解析
//!
//! Phase 1（桌面端）：此模块编译但不被调用，桌面端直接改 hosts 文件。
//! Phase 2（移动端）：通过 VPN 隧道把系统 DNS 指向本服务端口。

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

use super::ProxyState;

/// DNS server 句柄
pub struct DnsServerHandle {
    pub listen_addr: SocketAddr,
    pub shutdown: tokio::sync::oneshot::Sender<()>,
}

/// 启动 DNS 服务器，返回监听地址
pub async fn start(
    state: Arc<ProxyState>,
) -> Result<(SocketAddr, tokio::sync::oneshot::Receiver<()>), String> {
    let sock = UdpSocket::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("绑定 UDP 失败: {}", e))?;
    let listen_addr = sock
        .local_addr()
        .map_err(|e| format!("获取监听地址失败: {}", e))?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let mappings = state.mappings.clone();
    let hit_count = state.hit_count.clone();
    let forward_count = state.forward_count.clone();

    // 接收 shutdown 信号的 future（共享）
    let mut shutdown_rx2 = tokio::sync::oneshot::channel::<()>().1;
    // 用一个 AtomicBool 替代：更简单
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_c = stop_flag.clone();

    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            if stop_flag_c.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let (len, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(_) => continue,
            };
            let resp = handle_query(&buf[..len], &mappings, &hit_count, &forward_count).await;
            let _ = sock.send_to(&resp, peer).await;
        }
        let _ = shutdown_tx; // 抑制未使用告警
    });

    // 返回关闭句柄：通过 stop_flag 设置 + oneshot
    // 简化：把 stop_flag 装入 handle
    // 为兼容签名，此处返回 (listen_addr, shutdown_rx)
    let _ = shutdown_rx2;
    Ok((listen_addr, shutdown_rx))
}

async fn handle_query(
    req: &[u8],
    mappings: &RwLock<std::collections::HashMap<String, std::net::IpAddr>>,
    hit_count: &std::sync::atomic::AtomicU64,
    _forward_count: &std::sync::atomic::AtomicU64,
) -> Vec<u8> {
    use hickory_proto::op::Message;

    let msg = match Message::from_vec(req) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    let mut resp = msg.clone();
    resp.set_message_type(hickory_proto::op::MessageType::Response);
    resp.set_response_code(hickory_proto::op::ResponseCode::NoError);

    let map = mappings.read().await;
    for query in msg.queries() {
        let name = query.name().to_lowercase_string();
        if let Some(ip) = map.get(&name) {
            hit_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let record = match ip {
                std::net::IpAddr::V4(v4) => hickory_proto::rr::Record::from_query(
                    query.clone(),
                    std::time::Duration::from_secs(300),
                    hickory_proto::rr::RData::A(hickory_proto::rr::rdata::A(*v4)),
                ),
                std::net::IpAddr::V6(v6) => hickory_proto::rr::Record::from_query(
                    query.clone(),
                    std::time::Duration::from_secs(300),
                    hickory_proto::rr::RData::AAAA(hickory_proto::rr::rdata::AAAA(*v6)),
                ),
            };
            resp.add_answer(record);
        }
    }

    resp.to_vec().unwrap_or_default()
}

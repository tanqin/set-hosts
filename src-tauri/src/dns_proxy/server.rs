//! 本地 DNS 服务器：UDP + TCP 同端口监听
//!
//! - 命中映射表的查询：直接返回 A / AAAA 记录（TTL 60 秒，便于配置变更后快速生效）；
//! - 其它查询：原样转发给上游 DNS，并回传上游应答（超出 UDP 大小被截断时改用 TCP 重查）；
//! - 非法报文丢弃，转发失败返回 SERVFAIL（若有本地命中则仍返回本地应答）。

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_proto::rr::rdata::{A, AAAA};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::oneshot;

use super::{Counters, Mappings};

/// 映射记录 TTL（秒）
const MAPPING_TTL: u32 = 60;
/// 上游单次查询超时
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(4);
/// 单个 DNS 报文最大长度（含 EDNS0 缓冲）
const MAX_PACKET: usize = 4096;
/// DNS 报文头部固定长度
const HEADER_LEN: usize = 12;

/// 运行中的 DNS 服务器句柄
pub struct ServerHandle {
    /// 实际监听地址（端口为 0 时由系统分配）
    pub listen_addr: SocketAddr,
    /// 关闭信号发送端
    shutdown: Option<oneshot::Sender<()>>,
}

impl ServerHandle {
    /// 停止服务器：发送关闭信号后，后台任务自行退出
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// 服务器共享上下文（映射表与上游列表均为共享引用，可热更新）
#[derive(Clone)]
struct Context {
    mappings: Arc<RwLock<Mappings>>,
    upstream: Arc<RwLock<Vec<SocketAddr>>>,
    counters: Arc<Counters>,
}

/// 启动 DNS 服务器（UDP + TCP 同端口），返回句柄
pub async fn start(
    bind_addr: SocketAddr,
    mappings: Arc<RwLock<Mappings>>,
    upstream: Arc<RwLock<Vec<SocketAddr>>>,
    counters: Arc<Counters>,
) -> Result<ServerHandle, String> {
    let udp = UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| format!("监听 {} 失败（端口可能已被占用）: {}", bind_addr, e))?;
    let listen_addr = udp
        .local_addr()
        .map_err(|e| format!("获取监听地址失败: {}", e))?;

    // TCP 与 UDP 端口互不冲突；TCP 失败不致命，仅提供 UDP 服务
    let tcp = match TcpListener::bind(listen_addr).await {
        Ok(listener) => Some(listener),
        Err(e) => {
            log::warn!("DNS 代理 TCP 监听 {} 失败（仅提供 UDP 服务）: {}", listen_addr, e);
            None
        }
    };

    let ctx = Context {
        mappings,
        upstream,
        counters,
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // UDP 循环：每个请求独立任务处理，避免上游慢查询阻塞后续请求
    let udp_ctx = ctx.clone();
    let udp_task = tokio::spawn(async move {
        let sock = Arc::new(udp);
        let mut buf = vec![0u8; MAX_PACKET];
        loop {
            let (len, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("DNS 代理接收 UDP 报文失败: {}", e);
                    continue;
                }
            };
            if len == 0 {
                continue;
            }
            let request = buf[..len].to_vec();
            let sock = sock.clone();
            let ctx = udp_ctx.clone();
            tokio::spawn(async move {
                if let Some(response) = resolve(&request, &ctx).await {
                    let _ = sock.send_to(&response, peer).await;
                }
            });
        }
    });

    // TCP 循环：DNS over TCP 使用 2 字节长度前缀
    let tcp_task = tcp.map(|listener| {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _peer)) => {
                        let ctx = ctx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_tcp(stream, ctx).await {
                                log::debug!("DNS 代理 TCP 会话结束: {}", e);
                            }
                        });
                    }
                    Err(e) => log::warn!("DNS 代理接收 TCP 连接失败: {}", e),
                }
            }
        })
    });

    // 关闭信号到达后终止监听任务
    tokio::spawn(async move {
        let _ = shutdown_rx.await;
        udp_task.abort();
        if let Some(task) = tcp_task {
            task.abort();
        }
    });

    Ok(ServerHandle {
        listen_addr,
        shutdown: Some(shutdown_tx),
    })
}

/// 处理一条 TCP 会话（可连续处理多个查询）
async fn handle_tcp(mut stream: TcpStream, ctx: Context) -> Result<(), std::io::Error> {
    loop {
        let mut len_buf = [0u8; 2];
        stream.read_exact(&mut len_buf).await?;
        let len = u16::from_be_bytes(len_buf) as usize;
        if len == 0 {
            return Ok(());
        }

        let mut request = vec![0u8; len];
        stream.read_exact(&mut request).await?;

        let Some(response) = resolve(&request, &ctx).await else {
            return Ok(());
        };
        if response.len() > u16::MAX as usize {
            log::warn!("DNS 代理应答超过 TCP 长度上限，已丢弃");
            return Ok(());
        }

        stream
            .write_all(&(response.len() as u16).to_be_bytes())
            .await?;
        stream.write_all(&response).await?;
        stream.flush().await?;
    }
}

/// 处理单个 DNS 请求报文；返回 `None` 表示丢弃（非法报文）
async fn resolve(request: &[u8], ctx: &Context) -> Option<Vec<u8>> {
    let query = match Message::from_vec(request) {
        Ok(m) => m,
        Err(e) => {
            log::debug!("丢弃非法 DNS 报文: {}", e);
            return None;
        }
    };

    // 仅处理标准查询，其它（如动态更新、非查询报文）一律转发
    if query.message_type() != MessageType::Query || query.op_code() != OpCode::Query {
        return forward(request, ctx).await.ok();
    }

    let mut response = Message::new();
    response.set_id(query.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(OpCode::Query);
    response.set_recursion_desired(query.recursion_desired());
    response.set_recursion_available(true);
    response.set_authoritative(true);
    for q in query.queries() {
        response.add_query(q.clone());
    }

    let mut answered: u64 = 0;
    let mut need_forward = false;

    {
        let mappings = ctx.mappings.read().unwrap_or_else(|e| e.into_inner());
        for q in query.queries() {
            let name = normalize_name(q.name());
            let Some(ip) = mappings.get(name.as_str()) else {
                // 未命中映射：交给上游
                need_forward = true;
                continue;
            };

            match (q.query_type(), ip) {
                (RecordType::A, IpAddr::V4(v4)) => {
                    response.add_answer(Record::from_rdata(
                        q.name().clone(),
                        MAPPING_TTL,
                        RData::A(A(*v4)),
                    ));
                    answered += 1;
                }
                (RecordType::AAAA, IpAddr::V6(v6)) => {
                    response.add_answer(Record::from_rdata(
                        q.name().clone(),
                        MAPPING_TTL,
                        RData::AAAA(AAAA(*v6)),
                    ));
                    answered += 1;
                }
                // 映射存在但协议族不匹配：返回空应答（NOERROR + 无记录），
                // 避免继续向真实 DNS 查询后拿到未被改写的地址
                (RecordType::A, IpAddr::V6(_)) | (RecordType::AAAA, IpAddr::V4(_)) => {
                    answered += 1;
                }
                // 其它类型（CNAME / MX / TXT …）与 hosts 语义无关，交给上游
                _ => need_forward = true,
            }
        }
    }

    if answered > 0 {
        ctx.counters.hit.fetch_add(answered, Ordering::Relaxed);
    }

    if need_forward {
        ctx.counters.forward.fetch_add(1, Ordering::Relaxed);
        match forward(request, ctx).await {
            Ok(bytes) => return Some(bytes),
            Err(e) => {
                log::warn!("DNS 代理转发失败: {}", e);
                if answered == 0 {
                    response.set_response_code(ResponseCode::ServFail);
                    return response.to_vec().ok();
                }
            }
        }
    }

    response.set_response_code(ResponseCode::NoError);
    response.to_vec().ok()
}

/// 把请求转发给上游 DNS（UDP；应答被截断时改用 TCP 重查）
async fn forward(request: &[u8], ctx: &Context) -> Result<Vec<u8>, String> {
    let servers = ctx
        .upstream
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    if servers.is_empty() {
        return Err("未配置上游 DNS 服务器".to_string());
    }

    let mut last_err = "上游 DNS 无响应".to_string();
    for server in servers {
        let udp_response = match query_upstream_udp(request, server).await {
            Ok(resp) => resp,
            Err(e) => {
                last_err = e;
                continue;
            }
        };

        if !is_truncated(&udp_response) {
            return Ok(udp_response);
        }

        // 应答被截断：尝试 TCP 取完整应答，失败则退回截断应答（客户端会自行处理）
        match query_upstream_tcp(request, server).await {
            Ok(full) => return Ok(full),
            Err(e) => {
                log::debug!("上游 {} TCP 重查失败，返回截断应答: {}", server, e);
                return Ok(udp_response);
            }
        }
    }

    Err(last_err)
}

/// 向单个上游发起 UDP 查询
async fn query_upstream_udp(request: &[u8], server: SocketAddr) -> Result<Vec<u8>, String> {
    let bind: SocketAddr = if server.is_ipv4() {
        SocketAddr::from(([0, 0, 0, 0], 0))
    } else {
        SocketAddr::from(([0u16; 8], 0))
    };

    let sock = UdpSocket::bind(bind)
        .await
        .map_err(|e| format!("创建上游套接字失败: {}", e))?;
    // 使用 connect 让内核过滤来源地址，避免收到无关报文
    sock.connect(server)
        .await
        .map_err(|e| format!("连接上游 {} 失败: {}", server, e))?;
    sock.send(request)
        .await
        .map_err(|e| format!("发送查询到上游 {} 失败: {}", server, e))?;

    let mut buf = vec![0u8; MAX_PACKET];
    let len = match tokio::time::timeout(UPSTREAM_TIMEOUT, sock.recv(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(format!("接收上游 {} 应答失败: {}", server, e)),
        Err(_) => return Err(format!("上游 {} 超时", server)),
    };

    if len < HEADER_LEN {
        return Err(format!("上游 {} 返回的报文过短", server));
    }
    if buf[..2] != request[..2] {
        return Err(format!("上游 {} 应答事务 ID 不匹配", server));
    }

    Ok(buf[..len].to_vec())
}

/// 向单个上游发起 TCP 查询（DNS over TCP）
async fn query_upstream_tcp(request: &[u8], server: SocketAddr) -> Result<Vec<u8>, String> {
    let len = u16::try_from(request.len())
        .map_err(|_| "请求报文超过 TCP 长度上限".to_string())?;

    let fut = async move {
        let mut stream = TcpStream::connect(server)
            .await
            .map_err(|e| format!("连接上游 {} 失败: {}", server, e))?;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| format!("发送查询到上游 {} 失败: {}", server, e))?;
        stream
            .write_all(request)
            .await
            .map_err(|e| format!("发送查询到上游 {} 失败: {}", server, e))?;
        stream.flush().await.map_err(|e| e.to_string())?;

        let mut len_buf = [0u8; 2];
        stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let resp_len = u16::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; resp_len];
        stream.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
        Ok::<Vec<u8>, String>(buf)
    };

    tokio::time::timeout(UPSTREAM_TIMEOUT, fut)
        .await
        .map_err(|_| format!("上游 {} TCP 查询超时", server))?
}

/// 查询名规范化：小写并去掉末尾的点（与 hosts 中的写法一致）
fn normalize_name(name: &Name) -> String {
    super::normalize_domain(&name.to_ascii())
}

/// 判断应答是否被截断（TC 标志位：flags 第二字节的最高位起第 2 位）
fn is_truncated(response: &[u8]) -> bool {
    response.len() > 3 && (response[2] & 0x02) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns_proxy::{Counters, Mappings};
    use std::collections::HashMap;

    fn context(mappings: Mappings, upstream: Vec<SocketAddr>) -> Context {
        Context {
            mappings: Arc::new(RwLock::new(mappings)),
            upstream: Arc::new(RwLock::new(upstream)),
            counters: Arc::new(Counters::default()),
        }
    }

    fn query_bytes(domain: &str, record_type: RecordType) -> Vec<u8> {
        let mut msg = Message::new();
        msg.set_id(0x1234);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(true);
        let mut q = hickory_proto::op::Query::new();
        q.set_name(Name::from_ascii(format!("{}.", domain)).unwrap());
        q.set_query_type(record_type);
        msg.add_query(q);
        msg.to_vec().unwrap()
    }

    fn parse(response: &[u8]) -> Message {
        Message::from_vec(response).unwrap()
    }

    #[tokio::test]
    async fn answers_a_record_from_mappings() {
        let mut mappings = HashMap::new();
        mappings.insert("dev.local".to_string(), "10.0.0.5".parse::<IpAddr>().unwrap());
        let ctx = context(mappings, vec![]);

        let response = resolve(&query_bytes("dev.local", RecordType::A), &ctx)
            .await
            .expect("应返回应答");

        let msg = parse(&response);
        assert_eq!(msg.id(), 0x1234);
        assert_eq!(msg.message_type(), MessageType::Response);
        assert_eq!(msg.response_code(), ResponseCode::NoError);
        assert_eq!(msg.answers().len(), 1);
        assert_eq!(msg.answers()[0].ttl(), MAPPING_TTL);
        assert_eq!(
            msg.answers()[0].data(),
            &RData::A(A("10.0.0.5".parse().unwrap()))
        );
        assert_eq!(ctx.counters.hit.load(Ordering::Relaxed), 1);
        assert_eq!(ctx.counters.forward.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn answers_aaaa_record_from_ipv6_mapping() {
        let mut mappings = HashMap::new();
        mappings.insert("v6.local".to_string(), "::1".parse::<IpAddr>().unwrap());
        let ctx = context(mappings, vec![]);

        let response = resolve(&query_bytes("v6.local", RecordType::AAAA), &ctx)
            .await
            .expect("应返回应答");
        let msg = parse(&response);
        assert_eq!(msg.answers().len(), 1);
        assert_eq!(
            msg.answers()[0].data(),
            &RData::AAAA(AAAA("::1".parse().unwrap()))
        );
    }

    #[tokio::test]
    async fn empty_answer_when_family_does_not_match() {
        let mut mappings = HashMap::new();
        mappings.insert("v4.local".to_string(), "10.0.0.5".parse::<IpAddr>().unwrap());
        let ctx = context(mappings, vec![]);

        let response = resolve(&query_bytes("v4.local", RecordType::AAAA), &ctx)
            .await
            .expect("应返回应答");
        let msg = parse(&response);
        assert_eq!(msg.response_code(), ResponseCode::NoError);
        assert!(msg.answers().is_empty());
        assert_eq!(ctx.counters.hit.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn servfail_when_no_mapping_and_no_upstream() {
        let ctx = context(HashMap::new(), vec![]);

        let response = resolve(&query_bytes("unknown.local", RecordType::A), &ctx)
            .await
            .expect("应返回应答");
        let msg = parse(&response);
        assert_eq!(msg.response_code(), ResponseCode::ServFail);
        assert_eq!(ctx.counters.forward.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn ignores_malformed_request() {
        let ctx = context(HashMap::new(), vec![]);
        assert!(resolve(&[0u8, 1, 2], &ctx).await.is_none());
    }

    #[tokio::test]
    async fn serves_real_udp_queries() {
        let mut mappings = HashMap::new();
        mappings.insert("dev.local".to_string(), "10.0.0.5".parse::<IpAddr>().unwrap());

        let mappings = Arc::new(RwLock::new(mappings));
        let upstream = Arc::new(RwLock::new(Vec::new()));
        let counters = Arc::new(Counters::default());

        let mut handle = start(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            mappings,
            upstream,
            counters.clone(),
        )
        .await
        .expect("DNS 服务器应能启动");

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.connect(handle.listen_addr).await.unwrap();

        // 命中映射：返回 A 记录
        client
            .send(&query_bytes("dev.local", RecordType::A))
            .await
            .unwrap();
        let mut buf = vec![0u8; MAX_PACKET];
        let len = client.recv(&mut buf).await.unwrap();
        let msg = parse(&buf[..len]);
        assert_eq!(msg.answers().len(), 1);
        assert_eq!(counters.hit.load(Ordering::Relaxed), 1);

        // 未命中且无上游：SERVFAIL
        client
            .send(&query_bytes("unknown.local", RecordType::A))
            .await
            .unwrap();
        let len = client.recv(&mut buf).await.unwrap();
        assert_eq!(
            parse(&buf[..len]).response_code(),
            ResponseCode::ServFail
        );

        // 停止后端口应被释放
        handle.stop();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let rebound = UdpSocket::bind(handle.listen_addr).await;
        assert!(rebound.is_ok(), "停止后应能重新绑定原端口");
    }

    #[test]
    fn detects_truncated_response() {
        // 头部 12 字节：ID + flags(0x8000 = 标准应答) + …
        let normal = vec![0x12, 0x34, 0x80, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(!is_truncated(&normal));
        let truncated = vec![0x12, 0x34, 0x82, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(is_truncated(&truncated));
        assert!(!is_truncated(&[0x12]));
    }
}

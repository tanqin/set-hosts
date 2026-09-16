// 与 Rust models.rs 对齐的 TS 类型

export interface Profile {
  id: string
  name: string
  content: string
  enabled: boolean
  created_at: string
  last_applied_at: string | null
  /** 是否为远程 hosts（内容来自 URL 拉取，只读） */
  is_remote: boolean
  /** 远程 hosts 的 URL（仅远程 profile） */
  url: string | null
  /** 最近一次成功拉取时间 */
  last_fetch_at: string | null
  /** 远程 hosts 自动刷新间隔（秒），0 = 从不自动刷新 */
  auto_refresh_secs: number
}

export interface BackupRecord {
  id: string
  timestamp: string
  content: string
  source_path: string
}

export interface ProxyStatus {
  running: boolean
  /** 本地 DNS 服务器监听地址（如 127.0.0.1:5353） */
  listen_addr: string | null
  /** 命中映射并直接应答的查询数 */
  hit_count: number
  /** 转发到上游的查询数 */
  forward_count: number
  /** 当前生效的映射条数（来自所有启用 profile） */
  mapping_count: number
  /** 上游 DNS 服务器 */
  upstream: string[]
  /** 移动端 VPN 隧道是否已接管系统 DNS（桌面端恒为 false） */
  tunnel_active: boolean
}

export type ExportFormat = 'hosts' | 'json'

export interface ImportSummary {
  profile_count: number
  entry_count: number
}

export interface PlatformInfo {
  is_desktop: boolean
  is_mobile: boolean
  os: string
  hosts_path: string | null
}

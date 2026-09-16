// Tauri invoke 强类型封装

import { invoke } from '@tauri-apps/api/core'
import type {
  BackupRecord,
  ExportFormat,
  ImportSummary,
  PlatformInfo,
  Profile,
  ProxyStatus,
} from '../types/ipc'

export const getProfiles = (): Promise<Profile[]> => invoke('get_profiles')

export const getActiveProfile = (): Promise<string | null> =>
  invoke('get_active_profile')

export const setActiveProfile = (profileId: string): Promise<void> =>
  invoke('set_active_profile', { profileId })

export const createProfile = (name: string): Promise<Profile> =>
  invoke('create_profile', { name })

export const createRemoteProfile = (
  name: string,
  url: string,
  autoRefreshSecs = 0,
): Promise<Profile> => invoke('create_remote_profile', { name, url, autoRefreshSecs })

export const refreshRemoteProfile = (profileId: string): Promise<Profile> =>
  invoke('refresh_remote_profile', { profileId })

export const deleteProfile = (id: string): Promise<void> =>
  invoke('delete_profile', { id })

export const renameProfile = (id: string, name: string): Promise<void> =>
  invoke('rename_profile', { id, name })

export const getProfileContent = (profileId: string): Promise<string> =>
  invoke('get_profile_content', { profileId })

export const saveProfileContent = (profileId: string, content: string): Promise<void> =>
  invoke('save_profile_content', { profileId, content })

export const toggleProfile = (profileId: string): Promise<string> =>
  invoke('toggle_profile', { profileId })

export const applyProfile = (profileId: string): Promise<string> =>
  invoke('apply_profile', { profileId })

export const backupHosts = (): Promise<BackupRecord> => invoke('backup_hosts')

export const listBackups = (): Promise<BackupRecord[]> => invoke('list_backups')

export const restoreBackup = (backupId: string): Promise<string> =>
  invoke('restore_backup', { backupId })

export const getCurrentHostsContent = (): Promise<string> =>
  invoke('get_current_hosts_content')

export const exportConfig = (format: ExportFormat): Promise<string> =>
  invoke('export_config', { format })

export const importConfig = (
  content: string,
  format: ExportFormat,
): Promise<ImportSummary> => invoke('import_config', { content, format })

export const exportConfigToFile = (path: string, format: ExportFormat): Promise<void> =>
  invoke('export_config_to_file', { path, format })

export const importConfigFromFile = (path: string, format: ExportFormat): Promise<ImportSummary> =>
  invoke('import_config_from_file', { path, format })

/** 启动 DNS 代理；port / upstream 传入后会持久化到设置 */
export const startDnsProxy = (port?: number, upstream?: string): Promise<ProxyStatus> =>
  invoke('start_dns_proxy', { port: port ?? null, upstream: upstream ?? null })

export const stopDnsProxy = (): Promise<void> => invoke('stop_dns_proxy')

export const getProxyStatus = (): Promise<ProxyStatus> => invoke('get_proxy_status')

export const getPlatformInfo = (): Promise<PlatformInfo> => invoke('get_platform_info')

export const openHostsFolder = (): Promise<void> => invoke('open_hosts_folder')

export const getDataDir = (): Promise<string> => invoke('get_data_dir')

export const changeDataDir = (newDir: string): Promise<void> =>
  invoke('change_data_dir', { newDir })

export interface AppSettingsPayload {
  custom_data_dir: string | null
  language: string
  theme: string
  hide_on_startup: boolean
  proxy_enabled: boolean
  proxy_protocol: string
  proxy_host: string
  proxy_port: number
  remote_auto_refresh: boolean
  write_mode: string
  dns_proxy_auto_start: boolean
  dns_proxy_port: number
  dns_upstream: string
}

export const getAppSettings = (): Promise<AppSettingsPayload> => invoke('get_app_settings')

export const saveAppSettings = (params: {
  language?: string
  theme?: string
  hideOnStartup?: boolean
  proxyEnabled?: boolean
  proxyProtocol?: string
  proxyHost?: string
  proxyPort?: number
  remoteAutoRefresh?: boolean
  writeMode?: string
  dnsProxyAutoStart?: boolean
  dnsProxyPort?: number
  dnsUpstream?: string
}): Promise<void> =>
  invoke('save_app_settings', {
    language: params.language ?? null,
    theme: params.theme ?? null,
    hideOnStartup: params.hideOnStartup ?? null,
    proxyEnabled: params.proxyEnabled ?? null,
    proxyProtocol: params.proxyProtocol ?? null,
    proxyHost: params.proxyHost ?? null,
    proxyPort: params.proxyPort ?? null,
    remoteAutoRefresh: params.remoteAutoRefresh ?? null,
    writeMode: params.writeMode ?? null,
    dnsProxyAutoStart: params.dnsProxyAutoStart ?? null,
    dnsProxyPort: params.dnsProxyPort ?? null,
    dnsUpstream: params.dnsUpstream ?? null,
  })

// 开机自启（桌面端；移动端恒为 false / 空操作）
export const getAutostartStatus = (): Promise<boolean> => invoke('get_autostart_status')

export const setAutostart = (enabled: boolean): Promise<void> =>
  invoke('set_autostart', { enabled })

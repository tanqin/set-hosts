// Tauri invoke 强类型封装

import { invoke } from '@tauri-apps/api/core'
import type {
  BackupRecord,
  ExportFormat,
  ImportSummary,
  PlatformInfo,
  Profile,
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

/** 移动端：确保 VPN 隧道已接管系统 DNS（回到前台时自愈，已授权则不会再弹窗） */
export const ensureTunnel = (): Promise<boolean> => invoke('ensure_tunnel')

/** 移动端「再按一次退出」确认后退出应用（Android 会先停 VPN 隧道再终止进程） */
export const exitApp = (): Promise<void> => invoke('exit_app')

export const getPlatformInfo = (): Promise<PlatformInfo> => invoke('get_platform_info')

/**
 * 诊断报告：本地 DNS 服务器状态 + 映射内容 + 本机自测 + 原生隧道日志
 *
 * 「域名打不开但直接访问 IP 正常」这类问题涉及四条链路（隧道 / 本地服务器 /
 * 映射表 / 上游），所以后端一次性拼成一段可直接复制的文本，真机排查不必装 adb。
 */
export const getDiagnostics = (): Promise<string> => invoke('get_diagnostics')

/** 清空诊断日志（目前只有原生隧道日志需要清） */
export const clearDiagnostics = (): Promise<void> => invoke('clear_diagnostics')

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
  })

// 开机自启（桌面端；移动端恒为 false / 空操作）
export const getAutostartStatus = (): Promise<boolean> => invoke('get_autostart_status')

export const setAutostart = (enabled: boolean): Promise<void> =>
  invoke('set_autostart', { enabled })

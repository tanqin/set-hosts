import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  exportConfig,
  getAppSettings,
  getAutostartStatus,
  getCurrentHostsContent,
  getPlatformInfo,
  importConfig,
  saveAppSettings,
  setAutostart,
} from '../api/tauri'
import type { ExportFormat, PlatformInfo } from '../types/ipc'
import { ElMessage } from 'element-plus'
import { setLocale, t, type Locale } from '../i18n'

export type Theme = 'light' | 'dark'
export type ProxyProtocol = 'http' | 'https' | 'socks5'

export const useSettingsStore = defineStore('settings', () => {
  const platform = ref<PlatformInfo | null>(null)

  // 应用设置（选项页可即时修改）
  const language = ref<Locale>('zh-CN')
  const theme = ref<Theme>('light')
  const hideOnStartup = ref(false)
  const autoStart = ref(false)

  // HTTP 代理（拉取远程 hosts 用，SwitchHosts 风格）
  const proxyEnabled = ref(false)
  const proxyProtocol = ref<ProxyProtocol>('http')
  const proxyHost = ref('')
  const proxyPort = ref(0)
  const remoteAutoRefresh = ref(true)

  async function loadPlatform() {
    try {
      platform.value = await getPlatformInfo()
    } catch {
      // ignore
    }
  }

  /** 启动时加载设置并立即应用（语言 / 主题 / 代理） */
  async function loadAppSettings() {
    try {
      const s = await getAppSettings()
      language.value = s.language === 'en' ? 'en' : 'zh-CN'
      theme.value = s.theme === 'dark' ? 'dark' : 'light'
      hideOnStartup.value = s.hide_on_startup
      proxyEnabled.value = s.proxy_enabled
      proxyProtocol.value = (['http', 'https', 'socks5'].includes(s.proxy_protocol)
        ? s.proxy_protocol
        : 'http') as ProxyProtocol
      proxyHost.value = s.proxy_host
      proxyPort.value = s.proxy_port
      remoteAutoRefresh.value = s.remote_auto_refresh
    } catch {
      // 非 Tauri 环境（纯浏览器调试）使用默认值
    }
    applyLocale(language.value)
    applyTheme(theme.value)
  }

  function applyLocale(l: Locale) {
    setLocale(l)
  }

  function applyTheme(v: Theme) {
    document.documentElement.classList.toggle('dark', v === 'dark')
  }

  /** 切换语言：立即生效并持久化 */
  async function setLanguage(l: Locale) {
    language.value = l
    applyLocale(l)
    try {
      await saveAppSettings({ language: l })
    } catch (e: any) {
      ElMessage.error(t('options.saveFailed', { msg: e }))
    }
  }

  /** 切换主题：立即生效并持久化 */
  async function setTheme(v: Theme) {
    theme.value = v
    applyTheme(v)
    try {
      await saveAppSettings({ theme: v })
    } catch (e: any) {
      ElMessage.error(t('options.saveFailed', { msg: e }))
    }
  }

  /** 切换启动时隐藏：立即持久化（下次启动生效），成功时不弹提示 */
  async function setHideOnStartup(v: boolean) {
    hideOnStartup.value = v
    try {
      await saveAppSettings({ hideOnStartup: v })
    } catch (e: any) {
      ElMessage.error(t('options.saveFailed', { msg: e }))
    }
  }

  /** 查询系统开机自启状态（选项页打开时刷新） */
  async function loadAutoStartStatus() {
    try {
      autoStart.value = await getAutostartStatus()
    } catch {
      // ignore
    }
  }

  /** 切换开机自启：立即生效并持久化到系统，成功时不弹提示 */
  async function setAutoStart(v: boolean) {
    try {
      await setAutostart(v)
      autoStart.value = await getAutostartStatus()
    } catch (e: any) {
      // 失败时回读真实状态，避免 UI 与系统不一致
      await loadAutoStartStatus()
      ElMessage.error(t('options.autoStartFailed', { msg: e }))
    }
  }

  /** 保存代理设置（任一代理字段变更后调用，静默生效，仅失败提示） */
  async function saveProxySettings() {
    try {
      await saveAppSettings({
        proxyEnabled: proxyEnabled.value,
        proxyProtocol: proxyProtocol.value,
        proxyHost: proxyHost.value,
        proxyPort: proxyPort.value,
      })
    } catch (e: any) {
      ElMessage.error(t('options.saveFailed', { msg: e }))
    }
  }

  /** 切换启动时自动刷新远程 hosts：立即持久化，静默生效 */
  async function setRemoteAutoRefresh(v: boolean) {
    remoteAutoRefresh.value = v
    try {
      await saveAppSettings({ remoteAutoRefresh: v })
    } catch (e: any) {
      ElMessage.error(t('options.saveFailed', { msg: e }))
    }
  }

  async function exportData(format: ExportFormat): Promise<string> {
    return exportConfig(format)
  }

  async function importData(content: string, format: ExportFormat) {
    try {
      const summary = await importConfig(content, format)
      ElMessage.success(t('io.imported', { profiles: summary.profile_count, entries: summary.entry_count }))
      return summary
    } catch (e: any) {
      ElMessage.error(t('io.importFailed', { msg: e }))
      throw e
    }
  }

  async function readCurrentHosts(): Promise<string> {
    return getCurrentHostsContent()
  }

  return {
    platform,
    language,
    theme,
    hideOnStartup,
    autoStart,
    proxyEnabled,
    proxyProtocol,
    proxyHost,
    proxyPort,
    remoteAutoRefresh,
    loadPlatform,
    loadAppSettings,
    setLanguage,
    setTheme,
    setHideOnStartup,
    loadAutoStartStatus,
    setAutoStart,
    saveProxySettings,
    setRemoteAutoRefresh,
    exportData,
    importData,
    readCurrentHosts,
  }
})

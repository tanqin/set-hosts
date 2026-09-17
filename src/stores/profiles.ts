import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import {
  applyProfile,
  createProfile,
  createRemoteProfile,
  deleteProfile,
  getActiveProfile,
  getProfileContent,
  getProfiles,
  refreshRemoteProfile,
  renameProfile,
  saveProfileContent,
  setActiveProfile,
  toggleProfile,
} from '../api/tauri'
import type { Profile } from '../types/ipc'
import { ElMessage } from 'element-plus'
import { t } from '../i18n'
import { useSettingsStore } from './settings'

export const useProfilesStore = defineStore('profiles', () => {
  const settingsStore = useSettingsStore()
  const profiles = ref<Profile[]>([])
  const activeId = ref<string | null>(null)
  const loading = ref(false)

  const isMobile = computed(() => settingsStore.platform?.is_mobile ?? false)

  /**
   * 正在等待系统 VPN 授权结果的 profile。
   *
   * 移动端「配置全部关闭 → 打开某个配置」时，后端会弹出系统 VPN 授权弹窗，
   * 而授权结果是异步回传的；若用户点了拒绝，需要把刚打开的开关回滚（见
   * [`handleVpnConsentResult`]），否则界面显示「已启用」但映射并未生效。
   */
  let pendingConsentId: string | null = null

  async function load() {
    loading.value = true
    try {
      profiles.value = await getProfiles()
      activeId.value = await getActiveProfile()
    } catch (e: any) {
      ElMessage.error(t('profiles.loadFailed', { msg: e }))
    } finally {
      loading.value = false
    }
  }

  async function create(name: string) {
    try {
      const p = await createProfile(name)
      profiles.value.push(p)
      return p
    } catch (e: any) {
      ElMessage.error(t('profiles.createFailed', { msg: e }))
    }
  }

  /** 新增远程 hosts：立即拉取一次内容，成功后选中；autoRefreshSecs = 0 表示从不自动刷新 */
  async function createRemote(name: string, url: string, autoRefreshSecs = 0) {
    try {
      const p = await createRemoteProfile(name, url, autoRefreshSecs)
      profiles.value.push(p)
      return p
    } catch (e: any) {
      ElMessage.error(t('profiles.remoteCreateFailed', { msg: e }))
    }
  }

  /** 同步后台定时刷新的结果（由 remote-hosts-refreshed 事件触发，不弹提示） */
  function applyRefreshedRemote(updated: Profile) {
    const p = profiles.value.find((x) => x.id === updated.id)
    if (!p) return
    p.content = updated.content
    p.last_fetch_at = updated.last_fetch_at
    p.last_applied_at = updated.last_applied_at
  }

  /** 立即刷新远程 hosts 内容（若已启用则同时重新应用），返回最新内容 */
  async function refreshRemote(id: string): Promise<string | null> {
    const p = profiles.value.find((x) => x.id === id)
    if (!p?.is_remote) return null
    try {
      const updated = await refreshRemoteProfile(id)
      p.content = updated.content
      p.last_fetch_at = updated.last_fetch_at
      p.last_applied_at = updated.last_applied_at
      ElMessage.success(t('profiles.remoteRefreshed'))
      return updated.content
    } catch (e: any) {
      ElMessage.error(t('profiles.remoteRefreshFailed', { msg: e }))
      return null
    }
  }

  async function remove(id: string) {
    try {
      await deleteProfile(id)
      profiles.value = profiles.value.filter((p) => p.id !== id)
      if (activeId.value === id) {
        activeId.value = profiles.value[0]?.id ?? null
      }
    } catch (e: any) {
      ElMessage.error(t('profiles.deleteFailed', { msg: e }))
    }
  }

  async function rename(id: string, name: string) {
    try {
      await renameProfile(id, name)
      const p = profiles.value.find((x) => x.id === id)
      if (p) p.name = name
    } catch (e: any) {
      ElMessage.error(t('profiles.renameFailed', { msg: e }))
    }
  }

  async function select(id: string) {
    activeId.value = id
    await setActiveProfile(id)
  }

  async function loadContent(id: string): Promise<string> {
    try {
      return await getProfileContent(id)
    } catch (e: any) {
      ElMessage.error(t('profiles.contentLoadFailed', { msg: e }))
      return ''
    }
  }

  async function saveContent(id: string, content: string) {
    try {
      await saveProfileContent(id, content)
      const p = profiles.value.find((x) => x.id === id)
      if (p) p.content = content
    } catch (e: any) {
      ElMessage.error(t('profiles.saveFailed', { msg: e }))
    }
  }

  /**
   * 切换 profile 启用状态。
   *
   * 移动端没有 hosts 写入能力，映射靠「内置 DNS 服务器 + VPN 隧道」生效，因此
   * 从「全部关闭」切到「打开」时会申请一次系统 VPN 授权：授权成功即永久记住，
   * 之后不再打扰；被拒绝则回滚这次打开（[`handleVpnConsentResult`]）。
   */
  async function toggle(id: string) {
    const p = profiles.value.find((x) => x.id === id)
    if (!p) return
    const willEnable = !p.enabled
    // 本次是否为「全部关闭 → 开启」：只有这种转换才需要重新申请 VPN 授权
    const needsConsent =
      willEnable && isMobile.value && !profiles.value.some((x) => x.id !== id && x.enabled)
    // 授权结果异步回传，先记下来，避免回传早于本函数返回时找不到目标
    if (needsConsent) pendingConsentId = id
    try {
      await toggleProfile(id)
      p.enabled = willEnable
      // 用本地化文案替代后端返回的中文字符串，保证语言切换后提示一致
      ElMessage.success(willEnable ? t('profiles.enabled') : t('profiles.disabled'))
    } catch (e: any) {
      if (needsConsent) pendingConsentId = null
      ElMessage.error(t('profiles.toggleFailed', { msg: e }))
    }
  }

  /**
   * 处理系统 VPN 授权结果（由 `vpn-consent` 事件驱动）。
   *
   * 拒绝时把刚打开的开关回滚：映射实际没生效，开关就不该停在「已启用」；
   * 回滚后用户再次从关切到开，会重新弹出授权弹窗。
   */
  async function handleVpnConsentResult(granted: boolean) {
    const id = pendingConsentId
    pendingConsentId = null
    if (granted || !id) return
    const p = profiles.value.find((x) => x.id === id)
    if (!p?.enabled) return
    try {
      await toggleProfile(id)
      p.enabled = false
    } catch (e: any) {
      ElMessage.error(t('profiles.toggleFailed', { msg: e }))
    }
  }

  async function apply(id: string, silent = false) {
    try {
      await applyProfile(id)
      const p = profiles.value.find((x) => x.id === id)
      if (p) {
        p.enabled = true
        p.last_applied_at = new Date().toISOString()
      }
      activeId.value = id
      if (!silent) ElMessage.success(t('profiles.enabled'))
    } catch (e: any) {
      ElMessage.error(t('profiles.applyFailed', { msg: e }))
    }
  }

  return {
    profiles,
    activeId,
    loading,
    load,
    create,
    createRemote,
    applyRefreshedRemote,
    refreshRemote,
    remove,
    rename,
    select,
    loadContent,
    saveContent,
    toggle,
    handleVpnConsentResult,
    apply,
  }
})

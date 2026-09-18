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
  updateRemoteProfile,
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

  /** 编辑远程 hosts：修改名称 / URL / 自动刷新间隔（URL 变化时后端会重新拉取内容） */
  async function updateRemote(
    id: string,
    name: string,
    url: string,
    autoRefreshSecs = 0,
  ): Promise<Profile | null> {
    try {
      const updated = await updateRemoteProfile(id, name, url, autoRefreshSecs)
      const p = profiles.value.find((x) => x.id === id)
      if (p) {
        p.name = updated.name
        p.url = updated.url
        p.auto_refresh_secs = updated.auto_refresh_secs
        p.content = updated.content
        p.last_fetch_at = updated.last_fetch_at
        p.last_applied_at = updated.last_applied_at
      }
      ElMessage.success(t('profiles.remoteUpdated'))
      return updated
    } catch (e: any) {
      ElMessage.error(t('profiles.remoteUpdateFailed', { msg: e }))
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
   *
   * 提示时机：需要授权时不立即弹「已启用」，等授权结果回来再决定——
   * 通过才提示，用户取消 / 拒绝则什么都不弹（此时开关会被静默回滚）。
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
      if (needsConsent) {
        // 需要系统 VPN 授权时先不弹提示：授权结果由 handleVpnConsentResult 决定——
        // 通过才提示「已启用」，用户取消 / 拒绝则静默回滚，什么都不弹
        return
      }
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
   *
   * 已授权过的情况下系统不弹窗，原生层会主动补发一次 granted=true（见
   * `DnsVpn.start`），因此这里总能收到结果。
   */
  async function handleVpnConsentResult(granted: boolean) {
    const id = pendingConsentId
    pendingConsentId = null
    if (!id) return
    const p = profiles.value.find((x) => x.id === id)
    if (granted) {
      // 授权通过：映射确实生效了，补上开启时延迟显示的提示
      if (p?.enabled) ElMessage.success(t('profiles.enabled'))
      return
    }
    // 用户取消 / 拒绝：开关回滚，且不弹任何提示（成功提示此前就没弹）
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
    updateRemote,
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

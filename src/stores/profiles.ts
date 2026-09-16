import { defineStore } from 'pinia'
import { ref } from 'vue'
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

export const useProfilesStore = defineStore('profiles', () => {
  const profiles = ref<Profile[]>([])
  const activeId = ref<string | null>(null)
  const loading = ref(false)

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

  async function toggle(id: string) {
    try {
      await toggleProfile(id)
      const p = profiles.value.find((x) => x.id === id)
      if (p) p.enabled = !p.enabled
      // 用本地化文案替代后端返回的中文字符串，保证语言切换后提示一致
      ElMessage.success(p?.enabled ? t('profiles.enabled') : t('profiles.disabled'))
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
    apply,
  }
})

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  ArrowDown,
  ArrowRight,
  Close,
  Delete,
  Document,
  Edit,
  Grid,
  Plus,
  Refresh,
  RefreshRight,
  Setting,
  Upload,
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { Profile } from './types/ipc'
import { useProfilesStore } from './stores/profiles'
import { useSettingsStore } from './stores/settings'
import { ensureTunnel, exitApp, getCurrentHostsContent } from './api/tauri'
import { getElLocale, locale, t } from './i18n'
import HostsEditor from './components/HostsEditor.vue'
import BackupDrawer from './components/drawers/BackupDrawer.vue'
import ImportExportDrawer from './components/drawers/ImportExportDrawer.vue'
import OptionsDrawer from './components/drawers/OptionsDrawer.vue'
import DiagnosticsDrawer from './components/drawers/DiagnosticsDrawer.vue'
import AboutDrawer from './components/drawers/AboutDrawer.vue'

const profilesStore = useProfilesStore()
const settingsStore = useSettingsStore()

// Element Plus 组件文案随语言即时切换
const elLocale = computed(() => getElLocale(locale.value))

// 移动端：屏蔽依赖系统 hosts / 桌面文件对话框的功能入口
const isMobile = computed(() => settingsStore.platform?.is_mobile ?? false)

const currentContent = ref('')
const settingsMenuVisible = ref(false)

// 自动保存状态
type SaveStatus = 'idle' | 'saving' | 'saved' | 'error'
const saveStatus = ref<SaveStatus>('idle')
let saveTimer: ReturnType<typeof setTimeout> | null = null

// 系统 Hosts（只读展示，带语法高亮 + 托管块区分）
const systemHosts = ref('')
const systemHostsLoading = ref(false)
const systemHostsCollapsed = ref(false)

// ---- 系统 Hosts 区域高度：可拖拽调整，整个面板最大不超过应用高度的 50% ----
const SYSTEM_HOSTS_DEFAULT_HEIGHT = 160
const SYSTEM_HOSTS_MIN_HEIGHT = 60
const SYSTEM_HOSTS_HEADER_HEIGHT = 30
const SYSTEM_HOSTS_RESIZER_HEIGHT = 4

const appHeight = ref(window.innerHeight)
const systemHostsHeight = ref(SYSTEM_HOSTS_DEFAULT_HEIGHT)
const systemHostsDragging = ref(false)

/** 内容区最大高度 = 应用高度的 50% - 面板头部与拖拽条 */
const systemHostsMaxHeight = computed(() =>
  Math.max(
    SYSTEM_HOSTS_MIN_HEIGHT,
    Math.floor(appHeight.value * 0.5) - SYSTEM_HOSTS_HEADER_HEIGHT - SYSTEM_HOSTS_RESIZER_HEIGHT,
  ),
)

/** 内容区实际高度（受最大值收敛，窗口变小时自动回缩） */
const systemHostsContentHeight = computed(() =>
  Math.min(Math.max(systemHostsHeight.value, SYSTEM_HOSTS_MIN_HEIGHT), systemHostsMaxHeight.value),
)

function onSystemHostsResizeStart(e: PointerEvent) {
  e.preventDefault()
  const startY = e.clientY
  const startHeight = systemHostsContentHeight.value
  systemHostsDragging.value = true

  const onMove = (ev: PointerEvent) => {
    // 向上拖动变高
    const next = startHeight - (ev.clientY - startY)
    systemHostsHeight.value = Math.min(
      Math.max(next, SYSTEM_HOSTS_MIN_HEIGHT),
      systemHostsMaxHeight.value,
    )
  }
  const onEnd = () => {
    systemHostsDragging.value = false
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onEnd)
    window.removeEventListener('pointercancel', onEnd)
  }

  window.addEventListener('pointermove', onMove)
  window.addEventListener('pointerup', onEnd)
  window.addEventListener('pointercancel', onEnd)
}

function onWindowResize() {
  appHeight.value = window.innerHeight
  if (systemHostsHeight.value > systemHostsMaxHeight.value) {
    systemHostsHeight.value = systemHostsMaxHeight.value
  }
}

const MANAGED_START = '# >>> Set Hosts Managed >>>'
const MANAGED_END = '# <<< Set Hosts Managed <<<'

/** 转义 HTML */
function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

/** 单行语法着色（与编辑区一致：注释绿斜体 / IP 蓝 / 域名青） */
function highlightLine(line: string): string {
  const escaped = escapeHtml(line)
  const trimmed = line.trim()
  if (trimmed.startsWith('#')) {
    return `<span class="sh-comment">${escaped}</span>`
  }
  let main = escaped
  let comment = ''
  const hashIdx = escaped.indexOf('#')
  if (hashIdx >= 0) {
    main = escaped.slice(0, hashIdx)
    comment = escaped.slice(hashIdx)
  }
  const parts = main.split(/(\s+)/)
  const colored = parts
    .map((part, idx) => {
      if (idx === 0 && part && /^[\d.:a-fA-F]+$/.test(part)) {
        return `<span class="sh-ip">${part}</span>`
      }
      if (idx > 0 && part && !/^\s+$/.test(part) && !/^[\d.:a-fA-F]+$/.test(part)) {
        return `<span class="sh-domain">${part}</span>`
      }
      return part
    })
    .join('')
  return comment ? colored + `<span class="sh-comment">${comment}</span>` : colored
}

/** 全文渲染：仅 Set Hosts 托管块内的内容做语法着色，块外保持原样纯文本 */
const systemHostsHighlighted = computed(() => {
  const lines = systemHosts.value.split('\n')
  let inManaged = false
  return lines
    .map((line) => {
      const trimmed = line.trim()
      if (trimmed === MANAGED_START) {
        inManaged = true
        return highlightLine(line)
      }
      if (trimmed === MANAGED_END) {
        inManaged = false
        return highlightLine(line)
      }
      if (inManaged) {
        return highlightLine(line)
      }
      // 托管块外：原样输出（仅转义）
      return escapeHtml(line)
    })
    .join('\n')
})

async function loadSystemHosts() {
  // 移动端无法读取系统 hosts，不请求后端避免报错
  if (isMobile.value) return
  systemHostsLoading.value = true
  try {
    systemHosts.value = await getCurrentHostsContent()
  } catch (e: any) {
    systemHosts.value = t('app.systemHostsReadFailed', { msg: e })
  } finally {
    systemHostsLoading.value = false
  }
}

// 抽屉显隐
const drawers = ref({
  backup: false,
  importExport: false,
  options: false,
  diagnostics: false,
  about: false,
})

// ---- 移动端 Android 返回手势（边缘滑动 / 返回键）----
// Kotlin 侧拦截系统返回事件后调用 window.__onAndroidBack()（见 MainActivity.kt），
// 这里按「子抽屉 → 弹窗 → 设置菜单 → 双击退出」的顺序逐层返回。
let lastBackAt = 0
const BACK_EXIT_WINDOW_MS = 2000

/** 是否有 Element Plus 消息框打开（重命名 / 删除确认等；抽屉与对话框单独处理） */
function isMessageBoxOpen(): boolean {
  return !!document.querySelector('.el-message-box')
}

/** 退出前先落盘未保存的编辑内容 */
async function requestExitApp() {
  try {
    await flushSave()
  } catch {
    // 保存失败也要允许退出
  }
  try {
    await exitApp()
  } catch {
    // 非 Tauri 环境（纯浏览器调试）忽略
  }
}

/** Android 返回手势统一入口（返回事件总是被前端消费，不再直接退出应用） */
function handleAndroidBack(): boolean {
  // 1. 子抽屉（选项 / 诊断日志 / 关于等）打开 → 关闭抽屉
  //    移动端设置菜单始终保持显示在背后，关闭子抽屉后用户自然回到设置菜单
  const openKey = (Object.keys(drawers.value) as (keyof typeof drawers.value)[]).find(
    (k) => drawers.value[k],
  )
  if (openKey) {
    drawers.value[openKey] = false
    return true
  }
  // 2. 新增远程 hosts 对话框打开 → 关闭对话框
  if (remoteDialogVisible.value) {
    if (!remoteCreating.value) remoteDialogVisible.value = false
    return true
  }
  // 3. 消息框（确认 / 输入）打开 → 模拟 Esc 关闭
  if (isMessageBoxOpen()) {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape' }))
    return true
  }
  // 4. 设置菜单打开 → 关闭设置菜单
  if (settingsMenuVisible.value) {
    settingsMenuVisible.value = false
    return true
  }
  // 5. 首页：第一次返回弹出提示，2 秒内第二次返回才退出
  const now = Date.now()
  if (now - lastBackAt <= BACK_EXIT_WINDOW_MS) {
    lastBackAt = 0
    requestExitApp()
  } else {
    lastBackAt = now
    ElMessage({
      message: t('app.pressBackAgainToExit'),
      icon: '',
      customClass: 'mobile-center-message',
      duration: 2000,
    })
  }
  return true
}

// 移动端：打开子抽屉时不再关闭设置菜单（见 openDrawer），因此子抽屉关闭时设置菜单
// 仍然保持在背后，无需再"重新打开"——避免"关闭子抽屉 → 重新弹出设置菜单"的侧边闪现。

// 后台定时刷新完成 → 同步 profile 列表、编辑区与系统 Hosts 只读区
let unlistenRemoteRefreshed: (() => void) | null = null

async function setupRemoteRefreshListener() {
  try {
    unlistenRemoteRefreshed = await listen<Profile>('remote-hosts-refreshed', ({ payload }) => {
      profilesStore.applyRefreshedRemote(payload)
      if (profilesStore.activeId === payload.id) {
        currentContent.value = payload.content
      }
      if (payload.enabled) loadSystemHosts()
    })
  } catch {
    // 非 Tauri 环境（纯浏览器调试）忽略
  }
}

// 系统 VPN 授权结果（移动端把配置开关从关切到开时弹出）→ 拒绝则回滚开关
let unlistenVpnConsent: (() => void) | null = null

async function setupVpnConsentListener() {
  try {
    unlistenVpnConsent = await listen<{ granted: boolean }>('vpn-consent', ({ payload }) => {
      profilesStore.handleVpnConsentResult(payload?.granted === true)
    })
  } catch {
    // 非 Tauri 环境（纯浏览器调试）忽略
  }
}

onMounted(async () => {
  // Android 返回手势统一入口（MainActivity 的 OnBackPressedCallback 调用）
  ;(window as any).__onAndroidBack = () => handleAndroidBack()
  window.addEventListener('resize', onWindowResize)
  document.addEventListener('visibilitychange', onDocumentVisibilityChange)
  setupRemoteRefreshListener()
  setupVpnConsentListener()
  setupWindowControls()
  await settingsStore.loadPlatform()
  await profilesStore.load()
  loadSystemHosts()
  if (profilesStore.activeId) {
    currentContent.value = await profilesStore.loadContent(profilesStore.activeId)
  }
  // 移动端：启动后 / 回到前台时确保 VPN 隧道已接管系统 DNS
  ensureMobileTunnel()
})

onUnmounted(() => {
  window.removeEventListener('resize', onWindowResize)
  document.removeEventListener('visibilitychange', onDocumentVisibilityChange)
  unlistenRemoteRefreshed?.()
  unlistenVpnConsent?.()
  unlistenResized?.()
})

/**
 * 桌面端自定义窗口控制：原生标题栏已移除（decorations: false），
 * 最小化 / 最大化 / 关闭由顶部栏右侧的按钮实现。
 *
 * 关闭走 appWindow.close()：后端 on_window_event 会拦截并隐藏到托盘，
 * 与原生标题栏时代的行为一致。
 */
const appWindow = getCurrentWindow()
const isMaximized = ref(false)
let unlistenResized: (() => void) | null = null

function setupWindowControls() {
  if (isMobile.value) return
  void syncMaximized()
  appWindow
    .onResized(() => {
      // 拖拽调整大小的过程中会连续触发，isMaximized 本身很轻量，无需节流
      void syncMaximized()
    })
    .then((unlisten) => {
      unlistenResized = unlisten
    })
}

async function syncMaximized() {
  try {
    isMaximized.value = await appWindow.isMaximized()
  } catch {
    // 移动端 / 早期窗口未就绪时忽略
  }
}

function minimizeWindow() {
  void appWindow.minimize()
}

function toggleMaximizeWindow() {
  void appWindow.toggleMaximize()
}

function closeWindow() {
  // 桌面端后端会拦截 close 并隐藏到托盘；移动端不走这里
  void appWindow.close()
}

/**
 * 移动端自愈：重新回到前台时确保隧道仍接管着系统 DNS。
 *
 * VPN 授权由系统按包名永久记忆，已授权后这里不会再弹窗；若系统回收进程、
 * 或授权被其它 VPN 应用抢占，这里会静默重新接管。后端只在「存在已启用的
 * 配置、且用户没有拒绝过授权」时才会真正接管，因此不会反复打扰用户。
 */
function onDocumentVisibilityChange() {
  if (document.visibilityState === 'visible') ensureMobileTunnel()
}

async function ensureMobileTunnel() {
  if (!isMobile.value) return
  try {
    await ensureTunnel()
  } catch {
    // 忽略：自愈失败不影响正常使用，用户可在「选项 → 高级」点「重新授权」手动重试
  }
}

const activeProfile = computed(() =>
  profilesStore.profiles.find((p) => p.id === profilesStore.activeId),
)

const lineCount = computed(() => currentContent.value.split('\n').length)
const byteSize = computed(() => new Blob([currentContent.value]).size)

// 切换 profile 时加载内容
watch(
  () => profilesStore.activeId,
  async (id) => {
    if (id) {
      currentContent.value = await profilesStore.loadContent(id)
      saveStatus.value = 'idle'
    }
  },
)

async function handleSelect(id: string) {
  await flushSave()
  await profilesStore.select(id)
}

// 防抖自动保存并应用
function onContentChange(val: string) {
  currentContent.value = val
  saveStatus.value = 'saving'
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(() => {
    autoSave()
  }, 600)
}

async function autoSave() {
  if (!profilesStore.activeId) return
  const id = profilesStore.activeId
  const content = currentContent.value
  // 远程 hosts 内容只读，不做自动保存
  const profile = profilesStore.profiles.find((x) => x.id === id)
  if (profile?.is_remote) return
  try {
    await profilesStore.saveContent(id, content)
    // 若当前 profile 已启用，自动应用到系统 hosts
    const p = profilesStore.profiles.find((x) => x.id === id)
    if (p?.enabled) {
      await profilesStore.apply(id, true)
      loadSystemHosts()
    }
    saveStatus.value = 'saved'
    setTimeout(() => {
      if (saveStatus.value === 'saved') saveStatus.value = 'idle'
    }, 1500)
  } catch (e: any) {
    saveStatus.value = 'error'
    ElMessage.error(t('app.autoSaveError', { msg: e }))
  }
}

// 切换 profile 前立即落盘
async function flushSave() {
  if (saveTimer) {
    clearTimeout(saveTimer)
    saveTimer = null
  }
  if (saveStatus.value === 'saving') {
    await autoSave()
  }
}

/** 操作按钮（重命名/删除/启停/刷新）点击时，先切换编辑区到对应 profile */
async function ensureSelected(id: string) {
  if (profilesStore.activeId !== id) {
    await handleSelect(id)
  }
}

// 键盘快捷键 Ctrl+S 立即保存
function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault()
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
    autoSave()
  }
}
async function handleCreateProfile() {
  try {
    const { value } = await ElMessageBox.prompt('', t('app.addProfile'), {
      inputPlaceholder: t('app.profilePlaceholder'),
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
    })
    if (value?.trim()) {
      const p = await profilesStore.create(value.trim())
      if (p) await profilesStore.select(p.id)
    }
  } catch {
    // 取消
  }
}

// ---- 远程 hosts ----
const remoteDialogVisible = ref(false)
const remoteCreating = ref(false)
const remoteForm = ref({ name: '', url: '', autoRefresh: 0 })
/** 正在编辑的远程 profile id；null = 新增模式 */
const remoteEditingId = ref<string | null>(null)
const remoteDialogTitle = computed(() =>
  remoteEditingId.value ? t('app.editRemote') : t('app.addRemote'),
)

/** 自动刷新间隔选项（秒）：0 = 从不，默认选中 */
const autoRefreshOptions = [
  { value: 0, labelKey: 'app.autoRefresh.never' },
  { value: 60, labelKey: 'app.autoRefresh.1m' },
  { value: 300, labelKey: 'app.autoRefresh.5m' },
  { value: 900, labelKey: 'app.autoRefresh.15m' },
  { value: 3600, labelKey: 'app.autoRefresh.1h' },
  { value: 86400, labelKey: 'app.autoRefresh.24h' },
  { value: 604800, labelKey: 'app.autoRefresh.7d' },
]

function openRemoteDialog() {
  remoteEditingId.value = null
  remoteForm.value = { name: '', url: '', autoRefresh: 0 }
  remoteDialogVisible.value = true
}

/** 编辑远程 hosts：名称 / URL / 自动刷新三项均可修改 */
function openEditRemoteDialog(p: Profile) {
  remoteEditingId.value = p.id
  remoteForm.value = {
    name: p.name,
    url: p.url ?? '',
    autoRefresh: p.auto_refresh_secs ?? 0,
  }
  remoteDialogVisible.value = true
}

/** "+"下拉：本地 profile / 远程 hosts */
function handleAddCommand(command: string | number | object) {
  if (command === 'remote') openRemoteDialog()
  else handleCreateProfile()
}

async function submitCreateRemote() {
  const { name, url, autoRefresh } = remoteForm.value
  if (!url.trim()) {
    ElMessage.warning(t('app.remoteUrlRequired'))
    return
  }
  remoteCreating.value = true
  try {
    if (remoteEditingId.value) {
      // 编辑模式：更新名称 / URL / 自动刷新
      const updated = await profilesStore.updateRemote(
        remoteEditingId.value,
        name.trim(),
        url.trim(),
        autoRefresh,
      )
      if (updated) {
        if (profilesStore.activeId === updated.id) currentContent.value = updated.content
        remoteDialogVisible.value = false
        // URL 变化后系统 hosts 已被重写，底部只读区需重新读取
        loadSystemHosts()
      }
    } else {
      // 新增模式
      const p = await profilesStore.createRemote(name.trim() || url.trim(), url.trim(), autoRefresh)
      if (p) {
        await profilesStore.select(p.id)
        currentContent.value = p.content
        remoteDialogVisible.value = false
      }
    }
  } finally {
    remoteCreating.value = false
  }
}

/** 立即刷新远程 hosts；若当前正在查看则同步编辑区 */
async function handleRefreshRemote(id: string) {
  await ensureSelected(id)
  const content = await profilesStore.refreshRemote(id)
  if (content !== null && profilesStore.activeId === id) {
    currentContent.value = content
    saveStatus.value = 'idle'
  }
  loadSystemHosts()
}

async function handleRename(id: string, name: string) {
  await ensureSelected(id)
  try {
    const { value } = await ElMessageBox.prompt('', t('app.rename'), {
      inputValue: name,
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
    })
    if (value?.trim()) {
      await profilesStore.rename(id, value.trim())
    }
  } catch {
    // 取消
  }
}

async function handleDelete(id: string) {
  await ensureSelected(id)
  try {
    await ElMessageBox.confirm(t('app.deleteConfirm'), t('common.delete'), {
      type: 'warning',
      confirmButtonText: t('common.delete'),
      cancelButtonText: t('common.cancel'),
    })
    await profilesStore.remove(id)
    if (profilesStore.activeId) {
      currentContent.value = await profilesStore.loadContent(profilesStore.activeId)
    }
    loadSystemHosts()
  } catch {
    // 取消
  }
}

async function handleToggle(id: string) {
  await ensureSelected(id)
  await profilesStore.toggle(id)
  loadSystemHosts()
}

function openDrawer(key: keyof typeof drawers.value) {
  // 桌面端：点击菜单项时关闭设置菜单，仅展示子抽屉（保持原行为）
  // 移动端：设置菜单保持显示，el-drawer 默认 append-to-body 且 z-index 远高于
  //        设置菜单（z-index: 10），子抽屉会自然覆盖显示在其上层。
  //        这样子抽屉关闭时设置菜单始终保持在背后可见，不会出现"先关再开"的侧边闪现。
  if (!isMobile.value) {
    settingsMenuVisible.value = false
  }
  // 重置所有，再打开目标（同一时间只允许一个子抽屉显示）
  Object.keys(drawers.value).forEach((k) => {
    drawers.value[k as keyof typeof drawers.value] = false
  })
  drawers.value[key] = true
}

const menuGroups = computed(() => {
  const groups = [
    {
      title: t('app.menuGroup.system'),
      items: [
        { key: 'backup' as const, label: t('app.menu.backup'), icon: RefreshRight },
      ],
    },
    {
      title: t('app.menuGroup.data'),
      items: [
        { key: 'importExport' as const, label: t('app.menu.importExport'), icon: Upload },
      ],
    },
    {
      title: t('app.menuGroup.tools'),
      items: [
        { key: 'options' as const, label: t('app.menu.options'), icon: Grid },
        // 诊断日志在移动端尤其重要：真机上没有 adb 也能把隧道状态贴给开发者
        { key: 'diagnostics' as const, label: t('app.menu.diagnostics'), icon: Document },
        { key: 'about' as const, label: t('app.menu.about'), icon: Setting },
      ],
    },
  ]
  // 移动端：备份/还原、导入/导出依赖系统 hosts 与桌面文件对话框，屏蔽入口
  if (isMobile.value) {
    return groups
      .map((g) => ({
        ...g,
        items: g.items.filter((i) => i.key !== 'backup' && i.key !== 'importExport'),
      }))
      .filter((g) => g.items.length > 0)
  }
  return groups
})
</script>

<template>
  <el-config-provider :locale="elLocale">
    <div
      class="app"
      :class="{ resizing: systemHostsDragging, mobile: isMobile }"
      @keydown="onKeydown"
      tabindex="0"
    >
      <!-- 顶部栏（桌面端兼作窗口拖拽区与控制按钮区，原生标题栏已移除） -->
      <div class="titlebar" data-tauri-drag-region>
        <div class="titlebar-left" data-tauri-drag-region>
          <el-dropdown trigger="click" @command="handleAddCommand">
            <el-button text size="small" :icon="Plus" />
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="local">{{ t('app.addProfile') }}</el-dropdown-item>
                <el-dropdown-item command="remote">{{ t('app.addRemote') }}</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
          <img class="app-logo" src="/app-icon.png" alt="Set Hosts" draggable="false" />
          <span class="app-title" data-tauri-drag-region>Set Hosts</span>
        </div>
        <div class="titlebar-right" data-tauri-drag-region>
          <el-button
            text
            size="small"
            :icon="Setting"
            @click="settingsMenuVisible = !settingsMenuVisible"
          />
          <!-- 桌面端窗口控制：最小化 / 最大化 / 关闭（关闭 = 隐藏到托盘） -->
          <div v-if="!isMobile" class="window-controls">
            <button type="button" class="win-btn" aria-label="Minimize" @click="minimizeWindow">
              <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
                <path d="M0 5h10" stroke="currentColor" stroke-width="1" />
              </svg>
            </button>
            <button
              type="button"
              class="win-btn"
              aria-label="Maximize"
              @click="toggleMaximizeWindow"
            >
              <svg
                v-if="!isMaximized"
                width="10"
                height="10"
                viewBox="0 0 10 10"
                aria-hidden="true"
              >
                <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" />
              </svg>
              <svg v-else width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
                <rect x="0.5" y="2.5" width="7" height="7" fill="none" stroke="currentColor" />
                <path d="M2.5 2.5v-2h7v7h-2" fill="none" stroke="currentColor" />
              </svg>
            </button>
            <button type="button" class="win-btn win-close" aria-label="Close" @click="closeWindow">
              <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
                <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      <!-- 主体 -->
      <div class="body">
        <!-- 移动端：配置列表以横向 Tab 展示在编辑区上方，数量多时可横向滚动 -->
        <div
          v-if="isMobile && profilesStore.profiles.length"
          class="profile-tabs"
          :aria-label="t('app.mobileTabs')"
        >
          <button
            v-for="p in profilesStore.profiles"
            :key="p.id"
            type="button"
            class="profile-tab"
            :class="{ active: p.id === profilesStore.activeId, enabled: p.enabled }"
            @click="handleSelect(p.id)"
          >
            <span class="tab-dot" aria-hidden="true"></span>
            <span class="tab-name">{{ p.name }}</span>
            <span v-if="p.is_remote" class="tab-remote">{{ t('app.remoteTag') }}</span>
          </button>
        </div>

        <!-- 左侧 profile 列表（桌面端） -->
        <div v-if="!isMobile" class="sidebar">
          <div
            v-for="p in profilesStore.profiles"
            :key="p.id"
            class="profile-item"
            :class="{ active: p.id === profilesStore.activeId }"
          >
            <div class="profile-info" @click="handleSelect(p.id)">
              <span class="profile-name">
                {{ p.name }}
                <el-tag v-if="p.is_remote" size="small" type="primary" effect="plain" class="remote-tag">
                  {{ t('app.remoteTag') }}
                </el-tag>
              </span>
            </div>
            <div class="profile-actions" @click.stop>
              <el-tooltip v-if="p.is_remote" :content="t('app.refreshRemote')" placement="top">
                <el-button text size="small" :icon="Refresh" @click="handleRefreshRemote(p.id)" />
              </el-tooltip>
              <el-button
                text
                size="small"
                :icon="Edit"
                @click="p.is_remote ? openEditRemoteDialog(p) : handleRename(p.id, p.name)"
              />
              <el-button
                text
                size="small"
                type="danger"
                :icon="Delete"
                @click="handleDelete(p.id)"
              />
            </div>
            <el-switch
              :model-value="p.enabled"
              size="small"
              inline-prompt
              @change="handleToggle(p.id)"
            />
          </div>
        </div>

        <!-- 编辑区 -->
        <div class="main">
          <!-- 当前 profile 编辑器 -->
          <div class="editor-wrap">
            <div class="panel-header">
              <div class="panel-title">
                <span v-if="!isMobile">Hosts</span>
                <el-tag v-if="activeProfile && !isMobile" size="small" type="success" effect="plain">
                  {{ activeProfile.name }}
                </el-tag>
                <el-tag
                  v-if="activeProfile"
                  size="small"
                  :type="activeProfile.enabled ? 'success' : 'info'"
                  effect="light"
                >
                  <template v-if="isMobile">
                    {{ activeProfile.enabled ? t('app.enabledShort') : t('app.disabledShort') }}
                  </template>
                  <template v-else>
                    {{ activeProfile.enabled ? t('app.enabled') : t('app.disabled') }}
                  </template>
                </el-tag>
                <el-tooltip
                  v-if="activeProfile?.is_remote && activeProfile.url && !isMobile"
                  :content="activeProfile.url"
                  placement="top"
                >
                  <span class="remote-url">{{ activeProfile.url }}</span>
                </el-tooltip>
              </div>
              <el-button
                v-if="activeProfile?.is_remote && !isMobile"
                text
                size="small"
                :icon="Refresh"
                @click="handleRefreshRemote(activeProfile.id)"
              >
                {{ t('app.refreshRemote') }}
              </el-button>
              <!-- 移动端触屏无 hover，把配置管理入口集中到编辑区头部 -->
              <div v-if="isMobile && activeProfile" class="mobile-actions">
                <el-switch
                  :model-value="activeProfile.enabled"
                  size="large"
                  inline-prompt
                  @change="handleToggle(activeProfile.id)"
                />
                <el-button
                  v-if="activeProfile.is_remote"
                  text
                  :icon="Refresh"
                  @click="handleRefreshRemote(activeProfile.id)"
                />
                <el-button
                  text
                  :icon="Edit"
                  @click="
                    activeProfile.is_remote
                      ? openEditRemoteDialog(activeProfile)
                      : handleRename(activeProfile.id, activeProfile.name)
                  "
                />
                <el-button text type="danger" :icon="Delete" @click="handleDelete(activeProfile.id)" />
              </div>
            </div>
            <div class="editor-body">
              <HostsEditor
                v-if="activeProfile"
                :model-value="currentContent"
                :readonly="activeProfile.is_remote"
                @update:model-value="onContentChange"
              />
              <div v-else class="empty-hint">{{ t('app.emptyHint') }}</div>
              <div v-if="activeProfile?.is_remote && activeProfile.last_fetch_at" class="remote-fetch-time">
                {{ t('app.lastFetch', { time: new Date(activeProfile.last_fetch_at).toLocaleString() }) }}
              </div>
            </div>
          </div>
        </div>

        <!-- 设置菜单 -->
        <transition name="slide-right">
          <div v-if="settingsMenuVisible" class="settings-menu">
            <div class="settings-menu-header">
              <span>{{ t('app.settings') }}</span>
              <el-button text :icon="Close" @click="settingsMenuVisible = false" />
            </div>
            <div v-for="group in menuGroups" :key="group.title" class="menu-group">
              <div class="menu-group-title">{{ group.title }}</div>
              <div
                v-for="item in group.items"
                :key="item.key"
                class="menu-item"
                @click="openDrawer(item.key)"
              >
                <el-icon><component :is="item.icon" /></el-icon>
                <span>{{ item.label }}</span>
              </div>
            </div>
          </div>
        </transition>
      </div>

      <!-- 系统 Hosts 只读展示（桌面端；移动端无法读取，隐藏） -->
      <div v-if="!isMobile" class="system-hosts-panel" v-loading="systemHostsLoading">
        <div
          v-show="!systemHostsCollapsed"
          class="panel-resizer"
          :class="{ dragging: systemHostsDragging }"
          :title="t('app.systemHostsResizeHint')"
          @pointerdown="onSystemHostsResizeStart"
        ></div>
        <div class="panel-header">
          <div class="panel-title" @click="systemHostsCollapsed = !systemHostsCollapsed">
            <el-icon><component :is="systemHostsCollapsed ? ArrowRight : ArrowDown" /></el-icon>
            <span>{{ t('app.systemHosts') }}</span>
            <el-tag size="small" type="info" effect="plain">{{ t('app.readonly') }}</el-tag>
          </div>
        </div>
        <pre
          v-show="!systemHostsCollapsed"
          class="system-hosts-content"
          :style="{ height: systemHostsContentHeight + 'px' }"
          v-html="systemHostsHighlighted"
        ></pre>
      </div>

      <!-- 状态栏 -->
      <div class="statusbar">
        <span>{{ t('app.lines', { n: lineCount }) }}</span>
        <span>{{ byteSize }} B</span>
        <span class="save-status" :class="saveStatus">
          <template v-if="saveStatus === 'saving'">{{ t('status.saving') }}</template>
          <template v-else-if="saveStatus === 'saved'">{{ t('status.saved') }}</template>
          <template v-else-if="saveStatus === 'error'">{{ t('status.error') }}</template>
        </span>
        <span v-if="activeProfile?.last_applied_at">
          {{ t('app.lastApplied', { time: new Date(activeProfile.last_applied_at).toLocaleString() }) }}
        </span>
      </div>

      <!-- 抽屉 -->
      <BackupDrawer v-model:visible="drawers.backup" @restored="loadSystemHosts" />
      <ImportExportDrawer v-model:visible="drawers.importExport" />
      <OptionsDrawer v-model:visible="drawers.options" />
      <DiagnosticsDrawer v-model:visible="drawers.diagnostics" />
      <AboutDrawer v-model:visible="drawers.about" />

      <!-- 新增远程 hosts 对话框 -->
      <el-dialog
        v-model="remoteDialogVisible"
        :title="remoteDialogTitle"
        width="480px"
        :close-on-click-modal="!remoteCreating"
        :close-on-press-escape="!remoteCreating"
        :show-close="!remoteCreating"
      >
        <el-form label-position="top" @submit.prevent="submitCreateRemote">
          <el-form-item :label="t('app.remoteName')">
            <el-input
              v-model="remoteForm.name"
              :placeholder="t('app.remoteNamePlaceholder')"
              maxlength="100"
            />
          </el-form-item>
          <el-form-item :label="t('app.remoteUrl')">
            <el-input
              v-model="remoteForm.url"
              placeholder="https://example.com/hosts"
              @keyup.enter="submitCreateRemote"
            />
          </el-form-item>
          <div class="hint-text in-form">{{ t('app.remoteUrlHint') }}</div>
          <el-form-item :label="t('app.autoRefresh')">
            <el-select v-model="remoteForm.autoRefresh" style="width: 100%">
              <el-option
                v-for="opt in autoRefreshOptions"
                :key="opt.value"
                :label="t(opt.labelKey)"
                :value="opt.value"
              />
            </el-select>
          </el-form-item>
          <div class="hint-text in-form">{{ t('app.autoRefreshHint') }}</div>
        </el-form>
        <template #footer>
          <el-button :disabled="remoteCreating" @click="remoteDialogVisible = false">
            {{ t('common.cancel') }}
          </el-button>
          <el-button type="primary" :loading="remoteCreating" @click="submitCreateRemote">
            {{ remoteEditingId ? t('app.remoteSave') : t('app.remoteAdd') }}
          </el-button>
        </template>
      </el-dialog>
    </div>
  </el-config-provider>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  transition: background-color 0.2s ease, color 0.2s ease;
}

.titlebar {
  /* 高度 = 标题栏 + 顶部安全区（状态栏），避免移动端顶部按钮被系统状态栏压住点不到 */
  height: calc(36px + var(--safe-inset-top, 0px));
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--safe-inset-top, 0px) 8px 0;
  background: var(--el-fill-color-light);
  border-bottom: 1px solid var(--el-border-color);
  -webkit-user-select: none;
  user-select: none;
}

.titlebar-left,
.titlebar-right {
  display: flex;
  align-items: center;
  gap: 4px;
}

/* 应用 logo：原生标题栏移除后，在自定义标题栏展示 */
.app-logo {
  width: 20px;
  height: 20px;
  margin-right: 2px;
  border-radius: 4px;
  pointer-events: none;
  -webkit-user-drag: none;
}

/* 桌面端窗口控制按钮（Windows 风格：通栏、悬停变色、关闭悬停红色） */
.window-controls {
  display: flex;
  align-items: center;
  align-self: stretch;
  margin-left: 4px;
  -webkit-app-region: no-drag;
}

.win-btn {
  width: 44px;
  align-self: stretch;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  padding: 0;
  background: transparent;
  color: var(--el-text-color-primary);
  cursor: default;
  outline: none;
  -webkit-app-region: no-drag;
}

.win-btn:hover {
  background: var(--el-fill-color);
}

.win-btn:active {
  background: var(--el-fill-color-dark);
}

.win-close:hover {
  background: #e81123;
  color: #fff;
}

/* 桌面端标题栏「+」「设置」图标略大一点，便于识别与点击 */
.app:not(.mobile) .titlebar :deep(.el-button .el-icon) {
  font-size: 18px;
}

.app-title {
  font-size: 13px;
  font-weight: 600;
  margin-left: 4px;
  color: var(--el-text-color-regular);
}

.body {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
}

.sidebar {
  width: 180px;
  flex-shrink: 0;
  background: var(--el-fill-color-lighter);
  border-right: 1px solid var(--el-border-color);
  overflow-y: auto;
  padding: 4px;
}

.profile-item {
  display: flex;
  align-items: center;
  padding: 6px 8px;
  border-radius: 4px;
  cursor: pointer;
  margin-bottom: 2px;
}

.profile-item:hover {
  background: var(--el-fill-color);
}

.profile-item.active {
  background: var(--el-color-primary-light-9);
}

.profile-info {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.profile-name {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.remote-tag {
  flex-shrink: 0;
  transform: scale(0.9);
}

.remote-url {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remote-fetch-time {
  position: absolute;
  right: 12px;
  bottom: 8px;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  pointer-events: none;
  background: var(--el-bg-color);
  padding: 0 6px;
  border-radius: 3px;
}

.hint-text {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

/* 表单内的提示文本：紧贴上方输入框，并与下一个字段留出间距 */
.hint-text.in-form {
  margin: -12px 0 16px;
}

.profile-actions {
  display: none;
  gap: 0;
  flex-shrink: 0;
  /* 强制贴住右侧开关，不与名称抢空间 */
  margin-left: auto;
  margin-right: 2px;
}

/* 压缩图标按钮内边距，让图标紧挨开关 */
.profile-actions :deep(.el-button) {
  padding: 4px 5px;
  margin-left: 0;
}

.profile-item:hover .profile-actions {
  display: flex;
}

.main {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.system-hosts-panel {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-top: 1px solid var(--el-border-color);
  background: var(--el-fill-color-lighter);
  width: 100%;
}

/* 拖拽条：位于面板顶部，向上拖动放大只读区 */
.panel-resizer {
  height: 4px;
  flex-shrink: 0;
  cursor: ns-resize;
  background: transparent;
  transition: background-color 0.15s ease;
  touch-action: none;
}

.panel-resizer:hover,
.panel-resizer.dragging {
  background: var(--el-color-primary-light-5);
}

/* 拖拽中禁止选中文本，并保持光标样式 */
.app.resizing,
.app.resizing * {
  cursor: ns-resize !important;
  user-select: none !important;
}

.system-hosts-content {
  margin: 0;
  padding: 8px 14px;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.6;
  color: var(--el-text-color-regular);
  white-space: pre-wrap;
  word-break: break-all;
  min-height: 60px;
  overflow-y: auto;
  user-select: text;
  background: var(--el-fill-color-lighter);
}

/* 系统 Hosts 语法着色（与编辑区一致，仅托管块内应用） */
.system-hosts-content :deep(.sh-comment) {
  color: #859900;
  font-style: italic;
}

.system-hosts-content :deep(.sh-ip) {
  color: #268bd2;
}

.system-hosts-content :deep(.sh-domain) {
  color: #2aa198;
}

.editor-wrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-body {
  flex: 1;
  overflow: hidden;
  position: relative;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 12px;
  height: 30px;
  flex-shrink: 0;
  background: var(--el-fill-color-light);
  border-bottom: 1px solid var(--el-border-color);
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-regular);
  cursor: pointer;
}

.empty-hint {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--el-text-color-placeholder);
  font-size: 14px;
}

.settings-menu {
  position: absolute;
  top: 0;
  right: 0;
  width: 220px;
  height: 100%;
  background: var(--el-bg-color);
  border-left: 1px solid var(--el-border-color);
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.08);
  z-index: 10;
  overflow-y: auto;
}

.settings-menu-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  font-weight: 600;
  font-size: 14px;
}

.menu-group {
  padding: 8px 0;
  border-bottom: 1px solid var(--el-border-color-extra-light);
}

.menu-group-title {
  padding: 4px 12px;
  font-size: 11px;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
}

.menu-item:hover {
  background: var(--el-fill-color-light);
}

.statusbar {
  /* 高度 = 状态栏 + 底部安全区（全面屏手势条） */
  height: calc(24px + var(--safe-inset-bottom, 0px));
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 12px var(--safe-inset-bottom, 0px);
  background: var(--el-fill-color-light);
  border-top: 1px solid var(--el-border-color);
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

.save-status.saving {
  color: #e6a23c;
}

.save-status.saved {
  color: #67c23a;
}

.save-status.error {
  color: #f56c6c;
}

.slide-right-enter-active,
.slide-right-leave-active {
  transition: transform 0.2s ease;
}

.slide-right-enter-from,
.slide-right-leave-to {
  transform: translateX(100%);
}

/* ==========================================================================
   移动端适配
   ========================================================================== */

/* ---- 顶部栏：整体加高，图标放大到易点按的尺寸 ---- */
.app.mobile .titlebar {
  height: calc(52px + var(--safe-inset-top, 0px));
  padding: var(--safe-inset-top, 0px) 14px 0;
}

.app.mobile .titlebar :deep(.el-button) {
  width: 42px;
  height: 42px;
  font-size: 24px;
}

.app.mobile .titlebar :deep(.el-button .el-icon) {
  font-size: 24px;
}

.app.mobile .app-title {
  font-size: 16px;
  margin-left: 8px;
}

/* ---- 主体：配置列表从左侧栏改为编辑区上方的横向 Tab ---- */
.app.mobile .body {
  flex-direction: column;
}

.profile-tabs {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  overflow-x: auto;
  overflow-y: hidden;
  background: var(--el-fill-color-lighter);
  border-bottom: 1px solid var(--el-border-color);
  -webkit-overflow-scrolling: touch;
  scrollbar-width: thin;
  scrollbar-color: var(--el-border-color-darker) transparent;
}

/* 数量多时显示细滚动条，提示可横向滑动 */
.profile-tabs::-webkit-scrollbar {
  height: 5px;
}

.profile-tabs::-webkit-scrollbar-track {
  background: transparent;
}

.profile-tabs::-webkit-scrollbar-thumb {
  background: var(--el-border-color);
  border-radius: 3px;
}

.profile-tab {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  max-width: 66vw;
  padding: 9px 16px;
  border-radius: 999px;
  border: 1px solid var(--el-border-color);
  background: var(--el-bg-color);
  color: var(--el-text-color-regular);
  font-family: inherit;
  font-size: 14px;
  line-height: 1.2;
  cursor: pointer;
  transition: background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease;
}

.profile-tab.active {
  background: var(--el-color-primary-light-9);
  border-color: var(--el-color-primary-light-5);
  color: var(--el-color-primary);
  font-weight: 600;
}

.profile-tab .tab-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 小圆点标示该配置是否启用 */
.profile-tab .tab-dot {
  flex-shrink: 0;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--el-text-color-placeholder);
}

.profile-tab.enabled .tab-dot {
  background: var(--el-color-success);
}

.profile-tab .tab-remote {
  flex-shrink: 0;
  padding: 0 5px;
  border-radius: 3px;
  font-size: 11px;
  font-weight: 400;
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}

/* ---- 编辑区头部：状态标签 + 配置管理入口 ---- */
.app.mobile .panel-header {
  height: auto;
  min-height: 48px;
  padding: 6px 14px;
}

.app.mobile .panel-title {
  font-size: 13px;
  cursor: default;
}

.mobile-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.mobile-actions :deep(.el-button) {
  width: 38px;
  height: 38px;
  padding: 0;
  font-size: 20px;
  margin: 0;
}

.mobile-actions :deep(.el-button .el-icon) {
  font-size: 20px;
}

/* ---- 设置菜单：加宽并加大条目，方便触屏 ---- */
.app.mobile .settings-menu {
  width: 78vw;
  max-width: 320px;
}

.app.mobile .settings-menu-header {
  padding: 14px;
  font-size: 16px;
}

.app.mobile .menu-group-title {
  padding: 8px 14px;
  font-size: 12px;
}

.app.mobile .menu-item {
  padding: 14px;
  font-size: 15px;
}

/* ---- 状态栏：字号加大 + 左右留出更多边距，避免被圆角屏幕裁切 ---- */
.app.mobile .statusbar {
  height: auto;
  min-height: calc(36px + var(--safe-inset-bottom, 0px));
  flex-wrap: wrap;
  gap: 4px 18px;
  padding: 6px max(18px, var(--safe-inset-right, 0px))
    calc(6px + var(--safe-inset-bottom, 0px)) max(18px, var(--safe-inset-left, 0px));
  font-size: 13px;
}

/* ---- 移动端「再按一次退出」提示：屏幕正中、无图标、文字居中 ---- */
:global(.mobile-center-message) {
  top: 50% !important;
  left: 50% !important;
  transform: translate(-50%, -50%) !important;
}

:global(.mobile-center-message .el-message__icon) {
  display: none !important;
}

:global(.mobile-center-message .el-message__content) {
  flex: none;
  text-align: center;
  padding: 0;
  margin: 0 auto;
}
</style>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import {
  ArrowDown,
  ArrowRight,
  Close,
  Delete,
  Edit,
  Grid,
  Plus,
  Refresh,
  RefreshRight,
  Setting,
  Upload,
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import en from 'element-plus/es/locale/lang/en'
import { listen } from '@tauri-apps/api/event'
import type { Profile } from './types/ipc'
import { useProfilesStore } from './stores/profiles'
import { useSettingsStore } from './stores/settings'
import { getCurrentHostsContent } from './api/tauri'
import { locale, t } from './i18n'
import HostsEditor from './components/HostsEditor.vue'
import BackupDrawer from './components/drawers/BackupDrawer.vue'
import ImportExportDrawer from './components/drawers/ImportExportDrawer.vue'
import OptionsDrawer from './components/drawers/OptionsDrawer.vue'
import AboutDrawer from './components/drawers/AboutDrawer.vue'

const profilesStore = useProfilesStore()
const settingsStore = useSettingsStore()

// Element Plus 组件文案随语言即时切换
const elLocale = computed(() => (locale.value === 'zh-CN' ? zhCn : en))

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
  about: false,
})

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

onMounted(async () => {
  window.addEventListener('resize', onWindowResize)
  setupRemoteRefreshListener()
  await settingsStore.loadPlatform()
  await profilesStore.load()
  loadSystemHosts()
  if (profilesStore.activeId) {
    currentContent.value = await profilesStore.loadContent(profilesStore.activeId)
  }
})

onUnmounted(() => {
  window.removeEventListener('resize', onWindowResize)
  unlistenRemoteRefreshed?.()
})

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
  remoteForm.value = { name: '', url: '', autoRefresh: 0 }
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
    const p = await profilesStore.createRemote(name.trim() || url.trim(), url.trim(), autoRefresh)
    if (p) {
      await profilesStore.select(p.id)
      currentContent.value = p.content
      remoteDialogVisible.value = false
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
  settingsMenuVisible.value = false
  // 重置所有，再打开目标
  Object.keys(drawers.value).forEach((k) => {
    drawers.value[k as keyof typeof drawers.value] = false
  })
  drawers.value[key] = true
}

const menuGroups = computed(() => [
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
      { key: 'about' as const, label: t('app.menu.about'), icon: Setting },
    ],
  },
])
</script>

<template>
  <el-config-provider :locale="elLocale">
    <div class="app" :class="{ resizing: systemHostsDragging }" @keydown="onKeydown" tabindex="0">
      <!-- 顶部栏 -->
      <div class="titlebar">
        <div class="titlebar-left">
          <el-dropdown trigger="click" @command="handleAddCommand">
            <el-button text size="small" :icon="Plus" />
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="local">{{ t('app.addProfile') }}</el-dropdown-item>
                <el-dropdown-item command="remote">{{ t('app.addRemote') }}</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
          <span class="app-title">Set Hosts</span>
        </div>
        <div class="titlebar-right">
          <el-button
            text
            size="small"
            :icon="Setting"
            @click="settingsMenuVisible = !settingsMenuVisible"
          />
        </div>
      </div>

      <!-- 主体 -->
      <div class="body">
        <!-- 左侧 profile 列表 -->
        <div class="sidebar">
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
              <el-button text size="small" :icon="Edit" @click="handleRename(p.id, p.name)" />
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
                <span>Hosts</span>
                <el-tag v-if="activeProfile" size="small" type="success" effect="plain">
                  {{ activeProfile.name }}
                </el-tag>
                <el-tag
                  v-if="activeProfile"
                  size="small"
                  :type="activeProfile.enabled ? 'success' : 'info'"
                  effect="light"
                >
                  {{ activeProfile.enabled ? t('app.enabled') : t('app.disabled') }}
                </el-tag>
                <el-tooltip
                  v-if="activeProfile?.is_remote && activeProfile.url"
                  :content="activeProfile.url"
                  placement="top"
                >
                  <span class="remote-url">{{ activeProfile.url }}</span>
                </el-tooltip>
              </div>
              <el-button
                v-if="activeProfile?.is_remote"
                text
                size="small"
                :icon="Refresh"
                @click="handleRefreshRemote(activeProfile.id)"
              >
                {{ t('app.refreshRemote') }}
              </el-button>
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

      <!-- 系统 Hosts 只读展示（占满底部全宽，高度可拖拽调整） -->
      <div class="system-hosts-panel" v-loading="systemHostsLoading">
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
      <AboutDrawer v-model:visible="drawers.about" />

      <!-- 新增远程 hosts 对话框 -->
      <el-dialog
        v-model="remoteDialogVisible"
        :title="t('app.addRemote')"
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
            {{ t('app.remoteAdd') }}
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
  height: 36px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
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
  height: 24px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 12px;
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
</style>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { openHostsFolder, getDataDir, changeDataDir } from '../../api/tauri'
import { useSettingsStore, type WriteMode } from '../../stores/settings'
import { t, type Locale } from '../../i18n'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()

const settingsStore = useSettingsStore()

const activeTab = ref('general')
const dataDir = ref('')

// 移动端屏蔽桌面专属功能（开机自启 / 启动隐藏 / hosts 路径 / 数据目录迁移）
const isMobile = computed(() => settingsStore.platform?.is_mobile ?? false)

async function loadDataDir() {
  try {
    dataDir.value = await getDataDir()
  } catch {
    dataDir.value = t('advanced.getDataDirFailed')
  }
}

watch(
  () => props.visible,
  v => {
    if (v) {
      activeTab.value = 'general'
      loadDataDir()
      settingsStore.loadAutoStartStatus()
    }
  }
)

// 语言 / 主题 / 启动时隐藏：均由 settings store 处理，切换立即生效并持久化
function handleLanguageChange(val: string) {
  settingsStore.setLanguage(val as Locale)
}

function handleThemeChange(val: string) {
  settingsStore.setTheme(val === 'dark' ? 'dark' : 'light')
}

function handleHideOnStartupChange(val: string | number | boolean) {
  settingsStore.setHideOnStartup(Boolean(val))
}

function handleAutoStartChange(val: string | number | boolean) {
  settingsStore.setAutoStart(Boolean(val))
}

// 写入模式：立即持久化，下次写入系统 hosts 时生效
function handleWriteModeChange(val: string | number | boolean) {
  settingsStore.setWriteMode((val === 'overwrite' ? 'overwrite' : 'append') as WriteMode)
}

// ---- 代理设置（SwitchHosts 风格：拉取远程 hosts 用）----

function handleProxyEnabledChange(val: string | number | boolean) {
  settingsStore.proxyEnabled = Boolean(val)
  settingsStore.saveProxySettings()
}

function handleProxyProtocolChange(val: string) {
  settingsStore.proxyProtocol = (['http', 'https', 'socks5'].includes(val)
    ? val
    : 'http') as 'http' | 'https' | 'socks5'
  settingsStore.saveProxySettings()
}

function handleProxyHostChange(val: string) {
  settingsStore.proxyHost = val.trim()
  settingsStore.saveProxySettings()
}

function handleProxyPortChange(val: number | null) {
  settingsStore.proxyPort = val ?? 0
  settingsStore.saveProxySettings()
}

function handleRemoteAutoRefreshChange(val: string | number | boolean) {
  settingsStore.setRemoteAutoRefresh(Boolean(val))
}

async function handleOpenHostsFolder() {
  try {
    await openHostsFolder()
  } catch (e: any) {
    ElMessage.error(t('advanced.openFailed', { msg: e }))
  }
}

async function handleChangeDataDir() {
  try {
    const selected = await invoke('plugin:dialog|open', {
      options: {
        directory: true,
        title: t('advanced.selectDataDir'),
        defaultPath: dataDir.value || undefined
      }
    })
    if (!selected) return

    await changeDataDir(selected as string)
    dataDir.value = selected as string
    ElMessage.success(t('advanced.dataDirChanged'))
  } catch (e: any) {
    ElMessage.error(t('advanced.changeFailed', { msg: e }))
  }
}
</script>

<template>
  <el-drawer :model-value="visible" :title="t('options.title')" direction="rtl" size="480px" @update:model-value="(v: boolean) => emit('update:visible', v)">
    <el-tabs v-model="activeTab">
      <el-tab-pane :label="t('options.tab.general')" name="general">
        <el-form label-position="top" style="max-width: 360px">
          <el-form-item :label="t('options.language')">
            <el-select :model-value="settingsStore.language" style="width: 100%" @change="handleLanguageChange">
              <el-option :label="t('options.language.zhCN')" value="zh-CN" />
              <el-option :label="t('options.language.en')" value="en" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('options.theme')">
            <el-select :model-value="settingsStore.theme" style="width: 100%" @change="handleThemeChange">
              <el-option :label="t('options.theme.light')" value="light" />
              <el-option :label="t('options.theme.dark')" value="dark" />
            </el-select>
          </el-form-item>
          <el-form-item v-if="!isMobile" :label="t('options.hideOnStartup')">
            <el-switch :model-value="settingsStore.hideOnStartup" @change="handleHideOnStartupChange" />
            <div class="hint-text" style="margin: 0 4px">
              {{ t('options.hideOnStartupHint') }}
            </div>
          </el-form-item>
          <el-form-item v-if="!isMobile" :label="t('options.autoStart')">
            <el-switch :model-value="settingsStore.autoStart" @change="handleAutoStartChange" />
            <div class="hint-text" style="margin: 0 4px">
              {{ t('options.autoStartHint') }}
            </div>
          </el-form-item>
          <el-form-item v-if="!isMobile" :label="t('options.writeMode')">
            <el-radio-group :model-value="settingsStore.writeMode" @change="handleWriteModeChange">
              <el-radio value="append">{{ t('options.writeMode.append') }}</el-radio>
              <el-radio value="overwrite">{{ t('options.writeMode.overwrite') }}</el-radio>
            </el-radio-group>
            <div class="hint-text" style="margin-top: 4px">
              {{ t('options.writeMode.appendHint') }}
              <br />
              {{ t('options.writeMode.overwriteHint') }}
            </div>
          </el-form-item>
        </el-form>
      </el-tab-pane>
      <el-tab-pane :label="t('options.tab.proxy')" name="proxy">
        <el-form label-position="left" label-width="60px" style="max-width: 360px">
          <el-form-item>
            <el-checkbox
              :model-value="settingsStore.proxyEnabled"
              @change="handleProxyEnabledChange"
            >
              {{ t('options.proxy.use') }}
            </el-checkbox>
          </el-form-item>
          <el-form-item :label="t('options.proxy.protocol')">
            <el-select
              :model-value="settingsStore.proxyProtocol"
              :disabled="!settingsStore.proxyEnabled"
              style="width: 100%"
              @change="handleProxyProtocolChange"
            >
              <el-option label="HTTP" value="http" />
              <el-option label="HTTPS" value="https" />
              <el-option label="SOCKS5" value="socks5" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('options.proxy.host')">
            <el-input
              :model-value="settingsStore.proxyHost"
              :disabled="!settingsStore.proxyEnabled"
              :placeholder="t('options.proxy.hostPlaceholder')"
              @change="handleProxyHostChange"
            />
          </el-form-item>
          <el-form-item :label="t('options.proxy.port')">
            <el-input-number
              :model-value="settingsStore.proxyPort || undefined"
              :disabled="!settingsStore.proxyEnabled"
              :min="1"
              :max="65535"
              :controls="false"
              :placeholder="t('options.proxy.portPlaceholder')"
              style="width: 100%"
              @change="handleProxyPortChange"
            />
          </el-form-item>
        </el-form>
        <div class="hint-text" style="max-width: 360px">
          {{ t('options.proxy.hint') }}
        </div>

        <el-divider style="max-width: 360px" />

        <el-form label-position="top" style="max-width: 360px">
          <el-form-item :label="t('options.proxy.autoRefresh')">
            <el-switch
              :model-value="settingsStore.remoteAutoRefresh"
              @change="handleRemoteAutoRefreshChange"
            />
            <div class="hint-text" style="margin: 0 4px">
              {{ t('options.proxy.autoRefreshHint') }}
            </div>
          </el-form-item>
        </el-form>
      </el-tab-pane>
      <el-tab-pane :label="t('options.tab.advanced')" name="advanced">
        <!-- 平台信息 -->
        <div class="section-title">{{ t('advanced.platformInfo') }}</div>
        <el-descriptions :column="1" border class="info-desc">
          <el-descriptions-item :label="t('advanced.os')">
            {{ settingsStore.platform?.os ?? '—' }}
          </el-descriptions-item>
          <el-descriptions-item :label="t('advanced.platformType')">
            {{ settingsStore.platform?.is_mobile ? t('advanced.mobile') : t('advanced.desktop') }}
          </el-descriptions-item>
          <el-descriptions-item v-if="!isMobile" :label="t('advanced.hostsPath')">
            <span class="clickable-path" @click="handleOpenHostsFolder">
              {{ settingsStore.platform?.hosts_path ?? '—' }}
            </span>
          </el-descriptions-item>
        </el-descriptions>

        <!-- 数据存储位置 -->
        <div class="section-title" style="margin-top: 24px">{{ t('advanced.dataDir') }}</div>
        <el-descriptions :column="1" border class="info-desc">
          <el-descriptions-item :label="t('advanced.storageDir')">
            <div class="data-dir-row">
              <span class="data-dir-path" :title="dataDir">{{ dataDir || '—' }}</span>
              <el-link v-if="!isMobile" type="primary" :underline="false" @click="handleChangeDataDir">
                {{ t('advanced.change') }}
              </el-link>
            </div>
          </el-descriptions-item>
        </el-descriptions>
        <div class="hint-text">
          {{ t('advanced.dataDirHint') }}
        </div>
      </el-tab-pane>
    </el-tabs>
  </el-drawer>
</template>

<style scoped>
.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 10px;
}

.info-desc {
  margin-bottom: 4px;
}

.clickable-path {
  color: var(--el-color-primary);
  cursor: pointer;
  word-break: break-all;
}

.clickable-path:hover {
  text-decoration: underline;
}

.hint-text {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 6px;
  margin-bottom: 4px;
}

.data-dir-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.data-dir-path {
  word-break: break-all;
  font-size: 12px;
  color: var(--el-text-color-regular);
  flex: 1;
  min-width: 0;
}
</style>

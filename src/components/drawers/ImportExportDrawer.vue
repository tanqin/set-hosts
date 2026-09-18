<script setup lang="ts">
import { computed, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Download, FolderOpened, Upload } from '@element-plus/icons-vue'
import { save, open } from '@tauri-apps/plugin-dialog'
import { exportConfigToFile, importConfigFromFile } from '../../api/tauri'
import type { ExportFormat } from '../../types/ipc'
import { useProfilesStore } from '../../stores/profiles'
import { useSettingsStore } from '../../stores/settings'
import { t } from '../../i18n'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean]; imported: [] }>()

const profilesStore = useProfilesStore()
const settingsStore = useSettingsStore()

// 移动端窄屏按 50% 宽度会把表单挤到无法操作，移动端铺满整屏，桌面端保持侧栏式抽屉
const drawerSize = computed(() => (settingsStore.platform?.is_mobile ? '100%' : '50%'))

const tab = ref<'import' | 'export'>('export')
const exportFormat = ref<ExportFormat>('hosts')
const importFormat = ref<ExportFormat>('hosts')
const importing = ref(false)
const selectedFile = ref<string | null>(null)

const filterName = computed(() =>
  exportFormat.value === 'json' ? 'JSON' : t('io.hostsText'),
)
const exportExtensions = computed(() =>
  exportFormat.value === 'json' ? ['json'] : ['txt', 'hosts'],
)

async function handleExport() {
  const path = await save({
    title: t('io.exportToFile'),
    defaultPath: exportFormat.value === 'json' ? 'Set Hosts.json' : 'Set Hosts-hosts.txt',
    filters: [{ name: filterName.value, extensions: exportExtensions.value }],
  })
  if (!path) return
  try {
    await exportConfigToFile(path, exportFormat.value)
    ElMessage.success(t('io.exported'))
  } catch (e: any) {
    ElMessage.error(t('io.exportFailed', { msg: e }))
  }
}

async function handleSelectFile() {
  const hostsFilter = { name: t('io.hostsText'), extensions: ['txt', 'hosts'] }
  const jsonFilter = { name: 'JSON', extensions: ['json'] }
  const path = await open({
    title: t('io.selectFile'),
    multiple: false,
    directory: false,
    filters: importFormat.value === 'json' ? [jsonFilter, hostsFilter] : [hostsFilter, jsonFilter],
  })
  if (typeof path === 'string') {
    selectedFile.value = path
    // 根据扩展名自动切换导入格式
    importFormat.value = path.toLowerCase().endsWith('.json') ? 'json' : 'hosts'
  }
}

async function handleImport() {
  if (!selectedFile.value) return
  importing.value = true
  try {
    const summary = await importConfigFromFile(selectedFile.value, importFormat.value)
    selectedFile.value = null
    ElMessage.success(t('io.imported', { profiles: summary.profile_count, entries: summary.entry_count }))
  } catch (e: any) {
    ElMessage.error(t('io.importFailed', { msg: e }))
  } finally {
    // 配置已被整体替换，即使写系统 hosts 失败（提权被拒）也要重新拉取列表，
    // 否则界面还停在导入前的配置上，用户会以为没导入成功
    await profilesStore.load()
    // 导入的配置里可能有启用项并已写进系统 hosts，底部只读区要跟着重新读取
    emit('imported')
    importing.value = false
  }
}
</script>

<template>
  <el-drawer
    :model-value="visible"
    :title="t('io.title')"
    direction="rtl"
    :size="drawerSize"
    @update:model-value="(v: boolean) => emit('update:visible', v)"
  >
    <el-tabs v-model="tab">
      <el-tab-pane :label="t('io.tab.export')" name="export">
        <el-radio-group v-model="exportFormat" style="margin-bottom: 12px">
          <el-radio-button value="hosts">{{ t('io.hostsText') }}</el-radio-button>
          <el-radio-button value="json">JSON</el-radio-button>
        </el-radio-group>
        <el-alert
          type="info"
          :closable="false"
          style="margin-bottom: 12px"
        >
          {{ t('io.exportHintHosts') }}<br />
          {{ t('io.exportHintJson') }}
        </el-alert>
        <el-button type="primary" :icon="Download" @click="handleExport">
          {{ t('io.exportToFile') }}
        </el-button>
      </el-tab-pane>
      <el-tab-pane :label="t('io.tab.import')" name="import">
        <el-radio-group v-model="importFormat" style="margin-bottom: 12px">
          <el-radio-button value="hosts">{{ t('io.hostsText') }}</el-radio-button>
          <el-radio-button value="json">JSON</el-radio-button>
        </el-radio-group>
        <div style="display: flex; align-items: center; gap: 12px; margin-bottom: 12px">
          <el-button :icon="FolderOpened" @click="handleSelectFile">
            {{ t('io.selectFile') }}
          </el-button>
          <span class="selected-file">{{ selectedFile || t('io.noFileSelected') }}</span>
        </div>
        <el-button
          type="primary"
          :icon="Upload"
          :disabled="!selectedFile"
          :loading="importing"
          @click="handleImport"
        >
          {{ t('io.importBtn') }}
        </el-button>
      </el-tab-pane>
    </el-tabs>
  </el-drawer>
</template>

<style scoped>
.selected-file {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  word-break: break-all;
}
</style>

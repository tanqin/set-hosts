<script setup lang="ts">
import { computed, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Download, FolderOpened, Upload } from '@element-plus/icons-vue'
import { save, open } from '@tauri-apps/plugin-dialog'
import { exportConfigToFile, importConfigFromFile } from '../../api/tauri'
import type { ExportFormat } from '../../types/ipc'
import { useProfilesStore } from '../../stores/profiles'
import { t } from '../../i18n'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()

const profilesStore = useProfilesStore()

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
    defaultPath: exportFormat.value === 'json' ? 'set-hosts.json' : 'set-hosts-hosts.txt',
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
  const path = await open({
    title: t('io.selectFile'),
    multiple: false,
    directory: false,
    filters: [
      { name: t('io.hostsText'), extensions: ['txt', 'hosts'] },
      { name: 'JSON', extensions: ['json'] },
    ],
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
    await profilesStore.load()
    ElMessage.success(t('io.imported', { profiles: summary.profile_count, entries: summary.entry_count }))
  } catch (e: any) {
    ElMessage.error(t('io.importFailed', { msg: e }))
  } finally {
    importing.value = false
  }
}
</script>

<template>
  <el-drawer
    :model-value="visible"
    :title="t('io.title')"
    direction="rtl"
    size="50%"
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

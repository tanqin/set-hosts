<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { CopyDocument, Delete, Refresh } from '@element-plus/icons-vue'
import { t } from '../../i18n'
import { useSettingsStore } from '../../stores/settings'
import { clearDiagnostics, getDiagnostics } from '../../api/tauri'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()

const settingsStore = useSettingsStore()

// 报告很长（尤其隧道日志），移动端铺满整屏才好读
const drawerSize = computed(() => (settingsStore.platform?.is_mobile ? '100%' : '55%'))

const report = ref('')
const loading = ref(false)

async function refresh() {
  loading.value = true
  try {
    report.value = await getDiagnostics()
  } catch (e) {
    report.value = String(e)
  } finally {
    loading.value = false
  }
}

/**
 * 复制：先试 Clipboard API，失败再退回 execCommand。
 * Android WebView 上 clipboard.writeText 可能因权限或焦点被拒，
 * 而这份日志的唯一用途就是发给开发者，不能复制等于白做。
 */
async function copyAll() {
  if (!report.value) return
  try {
    await navigator.clipboard.writeText(report.value)
    ElMessage.success(t('diagnostics.copied'))
    return
  } catch {
    // 落到下面的兜底方案
  }

  const area = document.createElement('textarea')
  area.value = report.value
  area.style.position = 'fixed'
  area.style.top = '0'
  area.style.opacity = '0'
  document.body.appendChild(area)
  area.focus()
  area.select()
  let ok = false
  try {
    ok = document.execCommand('copy')
  } catch {
    ok = false
  }
  document.body.removeChild(area)

  if (ok) ElMessage.success(t('diagnostics.copied'))
  else ElMessage.warning(t('diagnostics.copyFailed'))
}

async function clearAll() {
  try {
    await ElMessageBox.confirm(t('diagnostics.clearConfirm'), t('diagnostics.clear'), {
      type: 'warning',
    })
  } catch {
    return
  }
  await clearDiagnostics()
  await refresh()
  ElMessage.success(t('diagnostics.cleared'))
}

// 打开时才生成：报告里全是实时状态，提前算没有意义
watch(
  () => props.visible,
  (visible) => {
    if (visible) refresh()
  },
  { immediate: true },
)
</script>

<template>
  <el-drawer
    :model-value="visible"
    :title="t('diagnostics.title')"
    direction="rtl"
    :size="drawerSize"
    @update:model-value="(v: boolean) => emit('update:visible', v)"
  >
    <div class="diagnostics">
      <div class="diagnostics-actions">
        <el-button size="small" :icon="Refresh" :loading="loading" @click="refresh">
          {{ t('diagnostics.refresh') }}
        </el-button>
        <el-button size="small" type="primary" :icon="CopyDocument" @click="copyAll">
          {{ t('diagnostics.copy') }}
        </el-button>
        <el-button size="small" :icon="Delete" @click="clearAll">
          {{ t('diagnostics.clear') }}
        </el-button>
      </div>

      <div class="diagnostics-hint">{{ t('diagnostics.hint') }}</div>

      <el-input
        v-model="report"
        class="diagnostics-report"
        type="textarea"
        readonly
        resize="none"
      />
    </div>
  </el-drawer>
</template>

<style scoped>
.diagnostics {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 8px;
}

.diagnostics-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.diagnostics-hint {
  font-size: 12px;
  line-height: 1.6;
  color: var(--el-text-color-secondary);
}

.diagnostics-report {
  flex: 1;
  min-height: 0;
}

.diagnostics-report :deep(.el-textarea__inner) {
  height: 100%;
  min-height: 240px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>

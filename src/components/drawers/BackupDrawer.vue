<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Clock, Document, RefreshLeft } from '@element-plus/icons-vue'
import { backupHosts, listBackups, restoreBackup } from '../../api/tauri'
import type { BackupRecord } from '../../types/ipc'
import { t } from '../../i18n'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{
  'update:visible': [boolean]
  /** 还原成功后触发，通知主页面刷新系统 hosts 展示 */
  restored: []
}>()

// 每次打开抽屉时刷新备份列表
watch(() => props.visible, (v) => {
  if (v) load()
})

const backups = ref<BackupRecord[]>([])
const loading = ref(false)
const viewDialog = ref(false)
const viewContent = ref('')

async function load() {
  loading.value = true
  try {
    backups.value = await listBackups()
  } catch (e: any) {
    ElMessage.error(t('backup.loadFailed', { msg: e }))
  } finally {
    loading.value = false
  }
}

onMounted(load)

async function create() {
  try {
    await backupHosts()
    ElMessage.success(t('backup.created'))
    await load()
  } catch (e: any) {
    ElMessage.error(t('backup.createFailed', { msg: e }))
  }
}

async function restore(id: string) {
  try {
    await ElMessageBox.confirm(t('backup.restoreConfirm'), t('backup.restoreTitle'), {
      type: 'warning',
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
    })
    await restoreBackup(id)
    ElMessage.success(t('backup.restored'))
    emit('restored')
  } catch {
    // 取消
  }
}
</script>

<template>
  <el-drawer
    class="backup-drawer-wrap"
    :model-value="visible"
    :title="t('backup.title')"
    direction="rtl"
    size="520px"
    @update:model-value="(v: boolean) => emit('update:visible', v)"
  >
    <div class="backup-drawer">
      <div class="backup-toolbar">
        <el-button type="primary" :icon="Clock" @click="create">{{ t('backup.create') }}</el-button>
        <span class="backup-count">{{ t('backup.count', { n: backups.length }) }}</span>
      </div>

      <!-- 仅列表区域滚动，顶部按钮固定 -->
      <div class="backup-list">
        <el-empty v-if="backups.length === 0" :description="t('backup.empty')" />

        <el-timeline v-else class="backup-timeline">
          <el-timeline-item
            v-for="b in backups"
            :key="b.id"
            :timestamp="new Date(b.timestamp).toLocaleString()"
            placement="top"
          >
            <el-card shadow="hover" class="backup-card" body-style="padding: 12px 16px">
              <div class="backup-card-row">
                <div class="backup-info">
                  <el-icon class="backup-info-icon"><Document /></el-icon>
                  <span class="backup-size">{{ t('backup.bytes', { n: b.content.length }) }}</span>
                  <span class="backup-path" :title="b.source_path">{{ b.source_path || '—' }}</span>
                </div>
                <div class="backup-actions">
                  <el-button size="small" :icon="Document" @click="viewContent = b.content; viewDialog = true">
                    {{ t('backup.view') }}
                  </el-button>
                  <el-button size="small" type="warning" :icon="RefreshLeft" @click="restore(b.id)">
                    {{ t('backup.restore') }}
                  </el-button>
                </div>
              </div>
            </el-card>
          </el-timeline-item>
        </el-timeline>
      </div>
    </div>

    <el-dialog v-model="viewDialog" :title="t('backup.content')" width="640px" append-to-body>
      <pre class="backup-viewer">{{ viewContent }}</pre>
    </el-dialog>
  </el-drawer>
</template>

<style scoped>
/* 关闭抽屉 body 自带滚动，改为内部列表区域滚动 */
.backup-drawer-wrap :deep(.el-drawer__body) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.backup-drawer {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.backup-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.backup-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.backup-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: 20px;
}

.backup-timeline {
  padding-left: 4px;
}

.backup-card {
  margin-bottom: 4px;
}

.backup-card-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.backup-info {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  min-width: 0;
  flex: 1;
}

.backup-info-icon {
  flex-shrink: 0;
}

.backup-size {
  flex-shrink: 0;
  color: var(--el-text-color-regular);
  font-weight: 500;
}

.backup-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  font-size: 12px;
}

.backup-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.backup-viewer {
  max-height: 400px;
  overflow: auto;
  background: var(--el-fill-color-lighter);
  padding: 12px;
  border-radius: 4px;
  font-size: 12px;
  margin: 0;
}
</style>

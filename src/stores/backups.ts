import { defineStore } from 'pinia'
import { ref } from 'vue'
import { backupHosts, listBackups, restoreBackup } from '../api/tauri'
import type { BackupRecord } from '../types/ipc'
import { ElMessage } from 'element-plus'

export const useBackupsStore = defineStore('backups', () => {
  const backups = ref<BackupRecord[]>([])
  const loading = ref(false)

  async function load() {
    loading.value = true
    try {
      backups.value = await listBackups()
    } catch (e: any) {
      ElMessage.error('加载备份列表失败: ' + e)
    } finally {
      loading.value = false
    }
  }

  async function create() {
    try {
      const rec = await backupHosts()
      backups.value.unshift(rec)
      ElMessage.success('已创建备份')
    } catch (e: any) {
      ElMessage.error('创建备份失败: ' + e)
    }
  }

  async function restore(id: string) {
    try {
      const msg = await restoreBackup(id)
      ElMessage.success(msg)
    } catch (e: any) {
      ElMessage.error('还原失败: ' + e)
    }
  }

  return { backups, loading, load, create, restore }
})

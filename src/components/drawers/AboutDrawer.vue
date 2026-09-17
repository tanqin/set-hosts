<script setup lang="ts">
import { computed } from 'vue'
import { t } from '../../i18n'
import { useSettingsStore } from '../../stores/settings'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()

const settingsStore = useSettingsStore()

// 移动端窄屏按 35% 宽度会把「版本 / 技术栈 / 支持平台」的标签列挤成竖排，
// 因此移动端铺满整屏，桌面端仍保持侧栏式抽屉。
const drawerSize = computed(() => (settingsStore.platform?.is_mobile ? '100%' : '35%'))
</script>

<template>
  <el-drawer
    :model-value="visible"
    :title="t('about.title')"
    direction="rtl"
    :size="drawerSize"
    @update:model-value="(v: boolean) => emit('update:visible', v)"
  >
    <div style="text-align: center; padding: 24px 0">
      <div style="font-size: 28px; font-weight: 600; margin-bottom: 8px">Set Hosts</div>
      <div style="color: var(--el-text-color-secondary); margin-bottom: 16px">
        {{ t('about.subtitle') }}
      </div>
      <el-descriptions :column="1" border style="text-align: left">
        <el-descriptions-item :label="t('about.version')">0.1.0</el-descriptions-item>
        <el-descriptions-item :label="t('about.techstack')">Tauri 2 · Rust · Vue3 · TypeScript · Element Plus</el-descriptions-item>
        <el-descriptions-item :label="t('about.platforms')">Linux · macOS · Windows · Android · iOS</el-descriptions-item>
      </el-descriptions>
      <div style="margin-top: 24px; color: var(--el-text-color-secondary); font-size: 12px">
        <p>{{ t('about.desktop') }}</p>
        <p>{{ t('about.mobile') }}</p>
      </div>
    </div>
  </el-drawer>
</template>

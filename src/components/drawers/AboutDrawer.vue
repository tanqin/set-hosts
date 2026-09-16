<script setup lang="ts">
import { t } from '../../i18n'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()
</script>

<template>
  <el-drawer
    :model-value="visible"
    :title="t('about.title')"
    direction="rtl"
    size="35%"
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

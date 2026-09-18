<script setup lang="ts">
import { computed } from 'vue'
import { open as openUrl } from '@tauri-apps/plugin-shell'
import { t } from '../../i18n'
import { useSettingsStore } from '../../stores/settings'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()

const settingsStore = useSettingsStore()

// 移动端窄屏按 35% 宽度会把「版本 / 技术栈 / 支持平台」的标签列挤成竖排，
// 因此移动端铺满整屏，桌面端仍保持侧栏式抽屉。
const drawerSize = computed(() => (settingsStore.platform?.is_mobile ? '100%' : '35%'))

const githubUrl = 'https://github.com/tanqin/set-hosts'

/** 用系统浏览器打开仓库地址（Tauri 内无法直接跳转外链，走 shell 插件） */
async function openGitHub() {
  try {
    await openUrl(githubUrl)
  } catch {
    // 纯浏览器调试环境：退回常规新标签页打开
    window.open(githubUrl, '_blank')
  }
}

/** 版本号在构建时由 vite 从 package.json 注入，随版本更新脚本自动同步 */
const appVersion = __APP_VERSION__
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
        <el-descriptions-item :label="t('about.version')">{{ appVersion }}</el-descriptions-item>
        <el-descriptions-item :label="t('about.techstack')">Tauri 2 · Rust · Vue3 · TypeScript · Element Plus</el-descriptions-item>
        <el-descriptions-item :label="t('about.platforms')">Linux · macOS · Windows · Android · iOS</el-descriptions-item>
        <el-descriptions-item :label="t('about.github')">
          <el-link
            type="primary"
            :underline="false"
            href="https://github.com/tanqin/set-hosts"
            target="_blank"
            rel="noopener"
            @click.prevent="openGitHub"
          >
            {{ githubUrl }}
          </el-link>
        </el-descriptions-item>
      </el-descriptions>
      <div style="margin-top: 24px; color: var(--el-text-color-secondary); font-size: 12px">
        <p>{{ t('about.desktop') }}</p>
        <p>{{ t('about.mobile') }}</p>
      </div>
    </div>
  </el-drawer>
</template>

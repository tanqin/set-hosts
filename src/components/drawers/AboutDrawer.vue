<script setup lang="ts">
import { computed } from 'vue'
import { open as openUrl } from '@tauri-apps/plugin-shell'
import { ElMessage } from 'element-plus'
import { t } from '../../i18n'
import { useSettingsStore } from '../../stores/settings'
import { openExternalUrl } from '../../api/tauri'

defineProps<{ visible: boolean }>()
const emit = defineEmits<{ 'update:visible': [boolean] }>()

const settingsStore = useSettingsStore()

// 移动端窄屏按 35% 宽度会把「版本 / 技术栈 / 支持平台」的标签列挤成竖排，
// 因此移动端铺满整屏，桌面端仍保持侧栏式抽屉。
const drawerSize = computed(() => (settingsStore.platform?.is_mobile ? '100%' : '35%'))

const githubUrl = 'https://github.com/tanqin/set-hosts'

/** 是否是 Android：必须走原生 ACTION_VIEW，否则 WebView 会自己加载外链 */
const isAndroid = computed(() => settingsStore.platform?.os === 'android')

/**
 * 用系统浏览器打开仓库地址。
 *
 * Android：调用原生 `open_external_url` Tauri 命令（JNI → ACTION_VIEW Intent）。
 *   不能用 `@tauri-apps/plugin-shell` 的 `open` 或 `window.open`：前者在某些机型上
 *   会静默失败；后者会让 WebView 自己加载外链——一旦跳走，应用自定义的返回手势
 *   （`window.__onAndroidBack`）会被冲掉，用户只能杀进程。
 * 桌面端 / iOS：继续使用 `@tauri-apps/plugin-shell` 的 `open`（iOS 上走系统浏览器）。
 * 纯浏览器调试环境：退回 `window.open`。
 */
async function openGitHub() {
  if (isAndroid.value) {
    try {
      await openExternalUrl(githubUrl)
    } catch (e: any) {
      ElMessage.error(t('about.openUrlFailed', { msg: e }))
    }
    return
  }
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
          <!--
            刻意不写 href / target="_blank"：Android WebView 下 <a target="_blank">
            会触发 shouldOverrideUrlLoading，可能直接把外链加载到本应用 WebView 里，
            冲掉 window.__onAndroidBack，导致返回手势失效（用户只能杀进程）。
            改为纯按钮形式，导航完全交给 click 处理函数按平台分发。
          -->
          <el-link type="primary" :underline="false" @click="openGitHub">
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

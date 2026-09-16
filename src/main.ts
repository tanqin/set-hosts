import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import App from './App.vue'
import { useSettingsStore } from './stores/settings'
import './styles/main.css'

const app = createApp(App)

// 注册所有 Element Plus 图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

app.use(createPinia())
app.use(ElementPlus)

// 挂载前加载设置（语言 / 主题），避免界面闪烁
const settingsStore = useSettingsStore()
settingsStore.loadAppSettings().finally(() => {
  app.mount('#app')
})

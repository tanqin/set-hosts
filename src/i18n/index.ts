// 轻量 i18n：响应式 locale + 多语言字典 + t()，切换立即生效（无需重启）

import { ref } from 'vue'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import zhTw from 'element-plus/es/locale/lang/zh-tw'
import en from 'element-plus/es/locale/lang/en'
import ja from 'element-plus/es/locale/lang/ja'
import ko from 'element-plus/es/locale/lang/ko'
import de from 'element-plus/es/locale/lang/de'
import fr from 'element-plus/es/locale/lang/fr'
import es from 'element-plus/es/locale/lang/es'
import ptBr from 'element-plus/es/locale/lang/pt-br'

import zhCNMessages from './locales/zh-CN'
import enMessages from './locales/en'
import zhTWMessages from './locales/zh-TW'
import jaMessages from './locales/ja'
import koMessages from './locales/ko'
import deMessages from './locales/de'
import frMessages from './locales/fr'
import esMessages from './locales/es'
import ptMessages from './locales/pt'

export type Locale =
  | 'zh-CN' // 简体中文
  | 'zh-TW' // 繁體中文
  | 'en' // English
  | 'ja' // 日本語
  | 'ko' // 한국어
  | 'de' // Deutsch
  | 'fr' // Français
  | 'es' // Español
  | 'pt' // Português (Brasil)

/** 语言元数据：选项菜单中显示的原生名称 + 对应的 Element Plus 语言包 */
export interface LocaleMeta {
  code: Locale
  /** 选项菜单中显示的语言原生名称（如"简体中文"、"日本語"） */
  label: string
  /** Element Plus 组件文案的语言包 */
  elLocale: any
}

export const LOCALES: LocaleMeta[] = [
  { code: 'zh-CN', label: '简体中文', elLocale: zhCn },
  { code: 'zh-TW', label: '繁體中文', elLocale: zhTw },
  { code: 'en', label: 'English', elLocale: en },
  { code: 'ja', label: '日本語', elLocale: ja },
  { code: 'ko', label: '한국어', elLocale: ko },
  { code: 'de', label: 'Deutsch', elLocale: de },
  { code: 'fr', label: 'Français', elLocale: fr },
  { code: 'es', label: 'Español', elLocale: es },
  { code: 'pt', label: 'Português (Brasil)', elLocale: ptBr },
]

export const locale = ref<Locale>('en')

const messages: Record<Locale, Record<string, string>> = {
  'zh-CN': zhCNMessages,
  en: enMessages,
  'zh-TW': zhTWMessages,
  ja: jaMessages,
  ko: koMessages,
  de: deMessages,
  fr: frMessages,
  es: esMessages,
  pt: ptMessages,
}

const SUPPORTED_LOCALES = LOCALES.map((l) => l.code)

/** 翻译：t('key', { n: 1 })，键不存在时回退 en，再回退键名 */
export function t(key: string, params?: Record<string, string | number>): string {
  let msg = messages[locale.value][key] ?? messages['en'][key] ?? key
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      msg = msg.split(`{${k}}`).join(String(v))
    }
  }
  return msg
}

/** 切换语言，立即生效 */
export function setLocale(l: Locale) {
  locale.value = l
}

/** 把任意字符串归一化为受支持的 Locale，未知值回退到 en */
export function normalizeLocale(l: string): Locale {
  return SUPPORTED_LOCALES.includes(l as Locale) ? (l as Locale) : 'en'
}

/** 查找当前 Locale 对应的 Element Plus 语言包（找不到回退 en） */
export function getElLocale(l: Locale) {
  return LOCALES.find((m) => m.code === l)?.elLocale ?? en
}

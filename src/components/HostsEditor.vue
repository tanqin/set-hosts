<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { t } from '../i18n'

const props = defineProps<{
  modelValue: string
  readonly?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [string]
}>()

const textareaRef = ref<HTMLTextAreaElement>()
const preRef = ref<HTMLPreElement>()
const gutterRef = ref<HTMLDivElement>()

const value = ref(normalize(props.modelValue))

watch(
  () => props.modelValue,
  (v) => {
    const nv = normalize(v)
    if (nv !== value.value) value.value = nv
  },
)

/** 统一换行符为 \n：\r\n 会让 <pre> 高亮层比 <textarea> 多出换行，导致行号错位 */
function normalize(v: string) {
  return v.replace(/\r\n/g, '\n')
}

function onInput() {
  emit('update:modelValue', value.value)
  syncScroll()
}

function syncScroll() {
  if (preRef.value && textareaRef.value) {
    preRef.value.scrollTop = textareaRef.value.scrollTop
    preRef.value.scrollLeft = textareaRef.value.scrollLeft
  }
  if (gutterRef.value && textareaRef.value) {
    gutterRef.value.scrollTop = textareaRef.value.scrollTop
  }
}

const lines = computed(() => value.value.split('\n').length)
const gutterLineNumbers = computed(() =>
  Array.from({ length: lines.value }, (_, i) => i + 1),
)

/**
 * 简易语法着色：将文本转为带颜色 span 的 HTML
 * - # 注释：灰色
 * - IP 地址：蓝色
 * - 域名：青色
 */
const highlighted = computed(() => {
  const text = value.value
  // 转义 HTML
  const escaped = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')

  const lines = escaped.split('\n')
  return lines
    .map((line) => {
      const trimmed = line.trim()
      if (trimmed.startsWith('#')) {
        // 整行注释
        return `<span class="comment">${line}</span>`
      }
      // 提取行尾注释
      let main = line
      let comment = ''
      const hashIdx = line.indexOf('#')
      if (hashIdx >= 0) {
        main = line.slice(0, hashIdx)
        comment = line.slice(hashIdx)
      }
      // 按空白拆分，第一个是 IP，其余是域名
      const parts = main.split(/(\s+)/)
      const colored = parts
        .map((part, idx) => {
          if (idx === 0 && part.match(/^[\d.:a-fA-F]+$/)) {
            return `<span class="ip">${part}</span>`
          }
          if (idx > 0 && part && !/^\s+$/.test(part) && !part.match(/^[\d.:a-fA-F]+$/)) {
            return `<span class="domain">${part}</span>`
          }
          return part
        })
        .join('')
      const commentHtml = comment
        ? `<span class="comment">${comment}</span>`
        : ''
      return colored + commentHtml
    })
    .join('\n')
})
</script>

<template>
  <div class="editor-wrapper">
    <!-- 行号 -->
    <div class="gutter" ref="gutterRef">
      <div v-for="n in gutterLineNumbers" :key="n" class="gutter-line">{{ n }}</div>
    </div>
    <!-- 编辑区 -->
    <div class="editor-area">
      <pre ref="preRef" class="highlight" v-html="highlighted"></pre>
      <textarea
        ref="textareaRef"
        class="textarea"
        v-model="value"
        :readonly="readonly"
        spellcheck="false"
        @input="onInput"
        @scroll="syncScroll"
        :placeholder="t('editor.placeholder')"
      />
    </div>
  </div>
</template>

<style scoped>
.editor-wrapper {
  display: flex;
  height: 100%;
  width: 100%;
  font-family: 'JetBrains Mono', 'Fira Code', Consolas, 'Courier New', monospace;
  font-size: 14px;
  line-height: 1.6;
  background: var(--el-bg-color);
}

.gutter {
  width: 48px;
  flex-shrink: 0;
  background: var(--el-fill-color-lighter);
  border-right: 1px solid var(--el-border-color-lighter);
  text-align: right;
  padding: 8px 8px 8px 0;
  color: var(--el-text-color-secondary);
  user-select: none;
  overflow: hidden;
}

.gutter-line {
  height: 1.6em;
}

.editor-area {
  position: relative;
  flex: 1;
  overflow: hidden;
  /* 右侧与滚动条/窗口边缘保留一点呼吸间隙 */
  margin-right: 8px;
}

.highlight,
.textarea {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 8px 12px;
  border: none;
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
  tab-size: 4;
  white-space: pre;
  word-wrap: normal;
  overflow: auto;
  box-sizing: border-box;
}

.highlight {
  pointer-events: none;
  color: var(--el-text-color-primary);
  z-index: 1;
}

.textarea {
  background: transparent;
  color: transparent;
  caret-color: var(--el-text-color-primary);
  z-index: 2;
  resize: none;
  outline: none;
}

.textarea::placeholder {
  color: var(--el-text-color-placeholder);
}

/* 语法颜色 */
:deep(.comment) {
  color: #859900;
  font-style: italic;
}
:deep(.ip) {
  color: #268bd2;
}
:deep(.domain) {
  color: #2aa198;
}
</style>

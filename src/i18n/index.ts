// 轻量 i18n：响应式 locale + 双语字典 + t()，切换立即生效（无需重启）

import { ref } from 'vue'

export type Locale = 'zh-CN' | 'en'

export const locale = ref<Locale>('zh-CN')

const messages: Record<Locale, Record<string, string>> = {
  'zh-CN': {
    // 通用
    'common.confirm': '确定',
    'common.cancel': '取消',
    'common.delete': '删除',

    // 主界面
    'app.settings': '设置',
    'app.menuGroup.system': '系统',
    'app.menuGroup.data': '数据',
    'app.menuGroup.tools': '工具',
    'app.menu.backup': '备份与还原',
    'app.menu.importExport': '导入 / 导出',
    'app.menu.options': '选项',
    'app.menu.about': '关于',
    'app.addProfile': '添加本地 Hosts',
    'app.profilePlaceholder': '如：开发环境',
    'app.rename': '重命名',
    'app.deleteConfirm': '确认删除该配置？',
    'app.enabled': '启用中·自动生效',
    'app.disabled': '未启用·仅保存',
    'app.emptyHint': '请选择或新建一个配置',
    'app.systemHosts': '系统 Hosts',
    'app.systemHostsReadFailed': '（读取失败：{msg}）',
    'app.readonly': '只读',
    'app.systemHostsResizeHint': '拖动调整高度（最大为窗口高度的一半）',
    'app.lines': '{n} 行',
    'app.lastApplied': '上次应用：{time}',
    'status.saving': '保存中…',
    'status.saved': '已自动保存',
    'status.error': '保存失败',
    'app.autoSaveError': '自动保存失败: {msg}',
    'app.addRemote': '添加远程 Hosts',
    'app.remoteTag': '远程',
    'app.refreshRemote': '刷新远程 Hosts',
    'app.remoteName': '名称（可选）',
    'app.remoteNamePlaceholder': '如：GitHub 加速，留空则使用 URL',
    'app.remoteUrl': 'URL',
    'app.remoteUrlHint': '将从该地址拉取 hosts 内容；若在选项中配置了代理，会通过代理拉取。',
    'app.remoteUrlRequired': '请输入 URL',
    'app.remoteAdd': '添加并拉取',
    'app.lastFetch': '上次拉取：{time}',
    'app.autoRefresh': '自动刷新',
    'app.autoRefreshHint': '按所选间隔在后台自动拉取最新内容（应用运行期间生效）。',
    'app.autoRefresh.never': '从不',
    'app.autoRefresh.1m': '1 分钟',
    'app.autoRefresh.5m': '5 分钟',
    'app.autoRefresh.15m': '15 分钟',
    'app.autoRefresh.1h': '1 小时',
    'app.autoRefresh.24h': '24 小时',
    'app.autoRefresh.7d': '7 天',
    'profiles.remoteCreateFailed': '添加远程 hosts 失败: {msg}',
    'profiles.remoteRefreshed': '远程 hosts 已刷新',
    'profiles.remoteRefreshFailed': '刷新远程 hosts 失败: {msg}',

    // 选项
    'options.title': '选项',
    'options.tab.general': '通用',
    'options.tab.proxy': '代理',
    'options.tab.advanced': '高级',
    'options.language': '语言',
    'options.language.zhCN': '简体中文',
    'options.language.en': 'English',
    'options.theme': '主题',
    'options.theme.light': '明亮',
    'options.theme.dark': '暗黑',
    'options.hideOnStartup': '启动时隐藏',
    'options.hideOnStartupHint': '启动时隐藏主窗口，在后台运行',
    'options.autoStart': '开机自启',
    'options.autoStartHint': '系统启动时自动运行 Set Hosts',
    'options.autoStartFailed': '设置开机自启失败: {msg}',
    'options.writeMode': '写入模式',
    'options.writeMode.append': '追加',
    'options.writeMode.overwrite': '覆盖',
    'options.writeMode.appendHint':
      '追加：新条目添加到系统 hosts 末尾的托管块，保留原有条目。',
    'options.writeMode.overwriteHint': '覆盖：用当前 profile 内容完全替换系统 hosts。',
    'options.saveFailed': '保存失败: {msg}',

    // 代理设置（SwitchHosts 风格，拉取远程 hosts 用）
    'options.proxy.use': '使用代理',
    'options.proxy.protocol': '协议',
    'options.proxy.host': '主机',
    'options.proxy.hostPlaceholder': '127.0.0.1',
    'options.proxy.port': '端口',
    'options.proxy.portPlaceholder': '8080',
    'options.proxy.hint': '代理仅用于拉取远程 hosts，不影响系统其他网络。',
    'options.proxy.autoRefresh': '启动时自动刷新',
    'options.proxy.autoRefreshHint': '应用启动时自动拉取已启用远程 hosts 的最新内容',

    // 高级
    'advanced.platformInfo': '平台信息',
    'advanced.os': '操作系统',
    'advanced.platformType': '平台类型',
    'advanced.desktop': '桌面端',
    'advanced.mobile': '移动端',
    'advanced.hostsPath': 'hosts 路径',
    'advanced.dataDir': '数据存储位置',
    'advanced.storageDir': '存储目录',
    'advanced.change': '更改',
    'advanced.dataDirHint': '备份数据、配置文件等存储在此目录下',
    'advanced.getDataDirFailed': '获取失败',
    'advanced.selectDataDir': '选择数据存储位置',
    'advanced.dataDirChanged': '数据存储位置已更改，重启后完全生效',
    'advanced.openFailed': '打开失败: {msg}',
    'advanced.changeFailed': '更改失败: {msg}',

    // 备份
    'backup.title': '备份与还原',
    'backup.create': '立即备份',
    'backup.count': '共 {n} 条备份（最多保留 50 条）',
    'backup.empty': '暂无备份',
    'backup.bytes': '{n} 字节',
    'backup.view': '查看',
    'backup.restore': '还原',
    'backup.content': '备份内容',
    'backup.restoreConfirm': '还原将覆盖当前系统 hosts，确认？',
    'backup.restoreTitle': '还原确认',
    'backup.restored': '已还原',
    'backup.created': '备份成功',
    'backup.createFailed': '备份失败: {msg}',
    'backup.loadFailed': '加载备份失败: {msg}',

    // 导入导出
    'io.title': '导入 / 导出',
    'io.tab.export': '导出',
    'io.tab.import': '导入',
    'io.hostsText': 'hosts 文本',
    'io.exportHintHosts': 'hosts 文本：导出当前激活 profile 的 hosts 内容。',
    'io.exportHintJson': 'JSON：导出全部配置（含所有 profile、备份）。',
    'io.exportToFile': '导出文件',
    'io.exported': '已导出到文件',
    'io.exportFailed': '导出失败: {msg}',
    'io.selectFile': '选择文件',
    'io.noFileSelected': '未选择文件',
    'io.importBtn': '导入文件',
    'io.imported': '导入成功：{profiles} 个配置，{entries} 条条目',
    'io.importFailed': '导入失败: {msg}',

    // 关于
    'about.title': '关于',
    'about.subtitle': '跨平台 Hosts 配置工具',
    'about.version': '版本',
    'about.techstack': '技术栈',
    'about.platforms': '支持平台',
    'about.desktop': '桌面端：直接修改系统 hosts 文件',
    'about.mobile': '移动端：通过本地 DNS 代理生效，无需 root/越狱',

    // 编辑器
    'editor.placeholder': '',

    // Profiles store
    'profiles.loadFailed': '加载配置失败: {msg}',
    'profiles.createFailed': '创建失败: {msg}',
    'profiles.deleteFailed': '删除失败: {msg}',
    'profiles.renameFailed': '重命名失败: {msg}',
    'profiles.contentLoadFailed': '加载内容失败: {msg}',
    'profiles.saveFailed': '保存失败: {msg}',
    'profiles.toggleFailed': '切换失败: {msg}',
    'profiles.applyFailed': '应用失败: {msg}',
    'profiles.enabled': '已启用',
    'profiles.disabled': '已禁用',
  },
  en: {
    // Common
    'common.confirm': 'OK',
    'common.cancel': 'Cancel',
    'common.delete': 'Delete',

    // Main window
    'app.settings': 'Settings',
    'app.menuGroup.system': 'System',
    'app.menuGroup.data': 'Data',
    'app.menuGroup.tools': 'Tools',
    'app.menu.backup': 'Backup & Restore',
    'app.menu.importExport': 'Import / Export',
    'app.menu.options': 'Options',
    'app.menu.about': 'About',
    'app.addProfile': 'Add Local Hosts',
    'app.profilePlaceholder': 'e.g. Dev environment',
    'app.rename': 'Rename',
    'app.deleteConfirm': 'Delete this profile?',
    'app.enabled': 'Enabled · Auto-applied',
    'app.disabled': 'Disabled · Saved only',
    'app.emptyHint': 'Select or create a profile',
    'app.systemHosts': 'System Hosts',
    'app.systemHostsReadFailed': '(Failed to read: {msg})',
    'app.readonly': 'Read-only',
    'app.systemHostsResizeHint': 'Drag to resize (max: half of the window height)',
    'app.lines': '{n} lines',
    'app.lastApplied': 'Last applied: {time}',
    'status.saving': 'Saving…',
    'status.saved': 'Auto-saved',
    'status.error': 'Save failed',
    'app.autoSaveError': 'Auto-save failed: {msg}',
    'app.addRemote': 'Add Remote Hosts',
    'app.remoteTag': 'Remote',
    'app.refreshRemote': 'Refresh remote hosts',
    'app.remoteName': 'Name (optional)',
    'app.remoteNamePlaceholder': 'e.g. GitHub加速; empty = URL',
    'app.remoteUrl': 'URL',
    'app.remoteUrlHint': 'Hosts content will be fetched from this URL; a proxy configured in Options will be used if enabled.',
    'app.remoteUrlRequired': 'Please enter a URL',
    'app.remoteAdd': 'Add & Fetch',
    'app.lastFetch': 'Last fetch: {time}',
    'app.autoRefresh': 'Auto refresh',
    'app.autoRefreshHint': 'Fetch the latest content in the background at the selected interval (while the app is running).',
    'app.autoRefresh.never': 'Never',
    'app.autoRefresh.1m': '1 minute',
    'app.autoRefresh.5m': '5 minutes',
    'app.autoRefresh.15m': '15 minutes',
    'app.autoRefresh.1h': '1 hour',
    'app.autoRefresh.24h': '24 hours',
    'app.autoRefresh.7d': '7 days',
    'profiles.remoteCreateFailed': 'Failed to add remote hosts: {msg}',
    'profiles.remoteRefreshed': 'Remote hosts refreshed',
    'profiles.remoteRefreshFailed': 'Failed to refresh remote hosts: {msg}',

    // Options
    'options.title': 'Options',
    'options.tab.general': 'General',
    'options.tab.proxy': 'Proxy',
    'options.tab.advanced': 'Advanced',
    'options.language': 'Language',
    'options.language.zhCN': '简体中文',
    'options.language.en': 'English',
    'options.theme': 'Theme',
    'options.theme.light': 'Light',
    'options.theme.dark': 'Dark',
    'options.hideOnStartup': 'Hide on startup',
    'options.hideOnStartupHint': 'Hide the main window on startup and run in the background',
    'options.autoStart': 'Launch at login',
    'options.autoStartHint': 'Automatically start Set Hosts when the system boots',
    'options.autoStartFailed': 'Failed to set launch at login: {msg}',
    'options.writeMode': 'Write mode',
    'options.writeMode.append': 'Append',
    'options.writeMode.overwrite': 'Overwrite',
    'options.writeMode.appendHint':
      'Append: new entries are added to the managed block at the end of the system hosts, keeping existing entries.',
    'options.writeMode.overwriteHint':
      'Overwrite: fully replace the system hosts with the current profile.',
    'options.saveFailed': 'Failed to save: {msg}',

    // Proxy settings (SwitchHosts style, for fetching remote hosts)
    'options.proxy.use': 'Use proxy',
    'options.proxy.protocol': 'Protocol',
    'options.proxy.host': 'Host',
    'options.proxy.hostPlaceholder': '127.0.0.1',
    'options.proxy.port': 'Port',
    'options.proxy.portPlaceholder': '8080',
    'options.proxy.hint': 'The proxy is only used to fetch remote hosts and does not affect other network traffic.',
    'options.proxy.autoRefresh': 'Auto refresh on startup',
    'options.proxy.autoRefreshHint': 'Automatically fetch the latest content of enabled remote hosts on launch',

    // Advanced
    'advanced.platformInfo': 'Platform info',
    'advanced.os': 'OS',
    'advanced.platformType': 'Platform type',
    'advanced.desktop': 'Desktop',
    'advanced.mobile': 'Mobile',
    'advanced.hostsPath': 'hosts path',
    'advanced.dataDir': 'Data location',
    'advanced.storageDir': 'Storage directory',
    'advanced.change': 'Change',
    'advanced.dataDirHint': 'Backups and config files are stored in this directory',
    'advanced.getDataDirFailed': 'Failed to load',
    'advanced.selectDataDir': 'Choose data directory',
    'advanced.dataDirChanged': 'Data location changed. Fully effective after restart',
    'advanced.openFailed': 'Failed to open: {msg}',
    'advanced.changeFailed': 'Failed to change: {msg}',

    // Backup
    'backup.title': 'Backup & Restore',
    'backup.create': 'Back up now',
    'backup.count': '{n} backups (up to 50 kept)',
    'backup.empty': 'No backups yet',
    'backup.bytes': '{n} bytes',
    'backup.view': 'View',
    'backup.restore': 'Restore',
    'backup.content': 'Backup content',
    'backup.restoreConfirm': 'Restoring will overwrite the current system hosts. Continue?',
    'backup.restoreTitle': 'Confirm restore',
    'backup.restored': 'Restored',
    'backup.created': 'Backup created',
    'backup.createFailed': 'Backup failed: {msg}',
    'backup.loadFailed': 'Failed to load backups: {msg}',

    // Import / Export
    'io.title': 'Import / Export',
    'io.tab.export': 'Export',
    'io.tab.import': 'Import',
    'io.hostsText': 'hosts text',
    'io.exportHintHosts': 'hosts text: export the hosts content of the active profile.',
    'io.exportHintJson': 'JSON: export all data (all profiles and backups).',
    'io.exportToFile': 'Export to file',
    'io.exported': 'Exported to file',
    'io.exportFailed': 'Export failed: {msg}',
    'io.selectFile': 'Select file',
    'io.noFileSelected': 'No file selected',
    'io.importBtn': 'Import from file',
    'io.imported': 'Imported {profiles} profiles, {entries} entries',
    'io.importFailed': 'Import failed: {msg}',

    // About
    'about.title': 'About',
    'about.subtitle': 'Cross-platform hosts manager',
    'about.version': 'Version',
    'about.techstack': 'Tech stack',
    'about.platforms': 'Platforms',
    'about.desktop': 'Desktop: edits the system hosts file directly',
    'about.mobile': 'Mobile: applies via a local DNS proxy, no root/jailbreak required',

    // Editor
    'editor.placeholder': '',

    // Profiles store
    'profiles.loadFailed': 'Failed to load profiles: {msg}',
    'profiles.createFailed': 'Failed to create: {msg}',
    'profiles.deleteFailed': 'Failed to delete: {msg}',
    'profiles.renameFailed': 'Failed to rename: {msg}',
    'profiles.contentLoadFailed': 'Failed to load content: {msg}',
    'profiles.saveFailed': 'Failed to save: {msg}',
    'profiles.toggleFailed': 'Failed to toggle: {msg}',
    'profiles.applyFailed': 'Failed to apply: {msg}',
    'profiles.enabled': 'Enabled',
    'profiles.disabled': 'Disabled',
  },
}

/** 翻译：t('key', { n: 1 })，键不存在时回退 zh-CN，再回退键名 */
export function t(key: string, params?: Record<string, string | number>): string {
  let msg = messages[locale.value][key] ?? messages['zh-CN'][key] ?? key
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

export function normalizeLocale(l: string): Locale {
  return l === 'en' ? 'en' : 'zh-CN'
}

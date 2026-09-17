# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

一个跨平台 hosts 配置管理工具（界面风格借鉴 SwitchHosts），基于 **Tauri 2 + Vue 3 + TypeScript + Rust** 构建：多配置（Profile）管理、远程 hosts 订阅、追加 / 覆盖两种写入模式、自动备份与还原、远程拉取支持代理；桌面端写入自动提权（Windows UAC）并刷新系统 DNS 缓存，移动端通过系统 VPN 授权接管 DNS 使映射生效。

## 功能特性

### 配置（Profile）管理

- **本地配置**：创建、重命名、删除、编辑 hosts 文本，一键启用 / 禁用 / 应用
- **远程配置**：订阅 `http(s)://` 地址，添加时立即拉取一次；名称、URL、自动刷新间隔随时可改（改 URL 会立即重新拉取，启用状态下还会重新应用到系统）
- **定时自动刷新**：每个远程配置可独立设置间隔（从不 / 1 分钟 / 5 分钟 / 15 分钟 / 1 小时 / 24 小时 / 7 天），后台按 30 秒粒度轮询；也可开启「启动时自动刷新」
- **多配置合并**：所有启用配置的条目合并写入系统 hosts 的「托管块」，互不冲突

### 写入系统 hosts（桌面端）

- **追加模式（默认）**：条目写入系统 hosts 末尾的托管块，**保留系统原有条目**（如 `127.0.0.1 localhost`）
- **覆盖模式**：用启用配置的内容**完全替换**系统 hosts（切换前请确认 `localhost` 等条目已写入配置）
- 托管块由 `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<` 标记包裹，应用时只增删该块，不触碰块外内容
- Windows 下自动提权写入（UAC），写入后自动刷新 DNS 缓存；macOS / Linux 写入各自的标准 hosts 路径

### 安全与备份

- **每次写入系统 hosts 前自动备份**，最多保留最近 50 份
- 支持手动备份、查看备份列表、一键还原任意备份

### 内置 DNS 代理（移动端映射方案）

桌面端直接改写系统 hosts；移动端无法改写系统 hosts，映射由「内置本地 DNS 服务器 + 系统 VPN 接管 DNS」生效：

- **全自动**：配置开关由「关」切到「开」时，后端自动启动本地 DNS 服务器并申请系统 VPN 授权，界面上没有相关配置项
- **默认监听 `127.0.0.1:5353`**（非特权端口，无需 root / 管理员）；端口被占用时自动回退到系统随机端口，保证服务一定能起来
- **命中映射直接应答** `A` / `AAAA`，TTL 1 秒（保证开关配置立即生效）；**其余查询转发上游** DNS（UDP，应答被截断时自动改用 TCP 重查）
- **上游自动探测**：自动探测系统 DNS（`/etc/resolv.conf`、`ipconfig`）；DNS 代理没有界面配置项，端口与上游均由后端自动决定
- **映射热更新**：配置增删改、启停、刷新远程 hosts、导入配置后立即同步，无需重启

### 远程拉取代理

- 拉取远程 hosts 时可配置 **HTTP / HTTPS / SOCKS5** 代理，仅作用于本应用的拉取请求，不影响系统其他网络

### 导入 / 导出

- **JSON**：完整配置（全部配置与备份）导出 / 导入
- **Hosts 文本**：导出当前配置的原始 hosts 文本，或把 hosts 文本导入为一个新配置
- 支持导出到文件、从文件导入

### 应用设置

- **界面语言**：简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) —— 共 9 种，切换立即生效
- 主题：浅色 / 深色
- 启动时隐藏到托盘、开机自启（桌面端）
- 写入模式、远程 hosts 拉取代理、启动时自动刷新远程 hosts
- 平台信息查看：操作系统、桌面 / 移动端、hosts 路径（可一键打开所在文件夹）
- 自定义数据存储目录（可迁移）

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | Vue 3（`<script setup>`）+ TypeScript + Pinia + Element Plus + Vite |
| 桌面 / 移动壳 | Tauri 2（tray-icon、dialog、shell、store、notification、autostart、single-instance 插件） |
| 后端 | Rust：tokio、hickory-proto（DNS）、reqwest（远程拉取；桌面用系统 TLS，移动端用 rustls）、serde、chrono、uuid |
| 移动端原生 | Android `VpnService`（`DnsVpnService.kt`，经 JNI 与 Rust 交互） |

## 开发

```bash
npm install                  # 安装依赖
npm run tauri dev            # 桌面端开发模式
npm run dev                  # 仅前端调试（无 Tauri 环境，部分功能不可用）
cd src-tauri && cargo test   # Rust 单元测试
```

移动端开发：

```bash
npm run tauri android dev    # Android（需 JDK 17+ 与 Android SDK / NDK）
npm run tauri ios dev        # iOS（需 macOS + Xcode）
```

## 构建与发布

一键打包当前系统能打的所有端（桌面端 + Android，macOS 上再加 iOS，各端互不影响）：

```bash
npm run build:all
```

| 命令 | 说明 |
|---|---|
| `npm run build:check` | 体检打包环境（JDK / Android SDK / NDK / rustup target），不构建 |
| `npm run build:desktop` | 当前系统桌面安装包（Windows `.exe` / `.msi`、macOS `.dmg`、Linux `.deb` / `.rpm` / `.AppImage`） |
| `npm run build:windows` / `build:macos` / `build:linux` | 显式指定系统；与本机不匹配时直接报错 |
| `npm run build:android` | Android APK（默认 arm64） |
| `npm run build:android:all` | 把全部 ABI 打进一个通用包 |
| `npm run build:android:split` | 每个 ABI 一个包（体积小） |
| `npm run build:android:debug` | 调试包（免签名、可调试） |
| `npm run build:android:aab` | Google Play 上架用的 AAB |
| `npm run build:ios` | iOS（需 macOS + Xcode） |

产物统一收集到项目根目录：`dist-desktop/`、`dist-apk/`、`dist-ios/`。

需要更细的控制时直接跑脚本（Windows PowerShell 下 `npm run xxx -- --flag value` 里带值的参数会被吞掉）：

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Android 打包环境：JDK 17+ 与 Android SDK（含 NDK）。脚本会自动探测常见安装位置，也可用 `--java-home` / `--sdk` 指定，或设置 `JAVA_HOME` / `ANDROID_HOME`；缺少 rustup target 时按 `npm run build:check` 的提示执行 `rustup target add aarch64-linux-android` 等命令补齐。

### 版本管理

版本号以 `package.json` 为唯一来源，`scripts/bump-version.mjs` 会同步 **8 处**：`package.json`、`package-lock.json`、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`、Android `tauri.properties`（`versionName` / `versionCode`）、Android 内嵌 `tauri.conf.json`、iOS `project.pbxproj`。

| 命令 | 说明 |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | 递增版本 → 同步全部文件 → 提交 → 打 `v*` 附注标签 |
| `npm run version:sync` | 不递增，仅把其它文件同步为 `package.json` 当前版本 |
| `npm run version:check` | 校验各处版本号是否一致（不一致时退出码非 0） |
| `npm run push` | `git push --follow-tags`，同时推送代码与标签 |

关于页显示的版本在构建时从 `package.json` 注入，打包各端时始终为当前最新版本号。

## 工作原理

### 桌面端：写入系统 hosts

1. 每个配置保存原始 hosts 文本（本地编辑或远程拉取）
2. 启用 / 禁用 / 应用时，后端收集**所有启用配置**的条目
3. 按写入模式生成新内容：追加 = 剥离旧托管块后追加新块；覆盖 = 整个文件仅保留新托管块
4. 提权写入前自动备份当前 hosts，写入后刷新 DNS 缓存

### 移动端：内置 DNS 代理 + VPN 接管

1. 把所有启用配置的条目编译成 `域名 → IP` 映射表（同一域名以先出现的配置为准）
2. 配置变更时重新编译映射表并热更新，无需重启服务
3. 收到 DNS 查询时：命中映射 → 直接返回 `A` / `AAAA`（协议族不匹配返回空应答，避免回落到真实 DNS）；未命中或 `CNAME` / `MX` 等类型 → 转发上游 DNS 并原样回传
4. Android 由 `DnsVpnService` 把系统 DNS 指向本地服务器，映射对全系统生效

## 平台支持

| 平台 | 状态 |
|---|---|
| Windows / macOS / Linux | ✅ 完整支持：写入系统 hosts、自动备份、远程订阅、导入导出 |
| Android | ✅ 支持：内置 DNS 代理 + `VpnService` 接管系统 DNS（首次开启配置会弹出一次系统 VPN 授权，系统按包名永久记住） |
| iOS | 🚧 部分支持：DNS 代理与前端已完成，`Network Extension` 隧道尚未接入（需付费开发者账号与 entitlement），隧道生效前映射不会作用于全系统 |

## 目录结构

```
src/            前端（Vue 3 + TS）：页面、组件、Pinia store、i18n（9 种语言）
src-tauri/      Rust 后端：命令层、hosts 解析、备份、DNS 代理、移动端原生桥接
scripts/        打包脚本与版本管理脚本
dist-desktop/   桌面端安装包产物
dist-apk/       Android 产物
```

## 注意事项

- Windows 首次写入系统 hosts 会弹出 UAC 提权确认，属正常现象
- 覆盖模式会移除系统原有 hosts 条目（含 `localhost`），如需保留请写入配置中；误操作可从「备份与还原」恢复
- 远程 hosts 单次内容上限 8MB、拉取超时 15 秒；内容应为标准 hosts 格式纯文本（每行 `IP 域名`），不支持 JSON
- Android 首次开启配置需授予 VPN 授权；拒绝后开关会自动回滚，再次开启会重新申请
- DNS 代理上游单次查询超时 4 秒，失败返回 `SERVFAIL`

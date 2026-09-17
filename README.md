# Set Hosts

一个跨平台 hosts 管理工具（SwitchHosts 风格），基于 **Tauri 2 + Vue 3 + TypeScript + Rust** 构建。支持多 profile 管理、远程 hosts 订阅、追加/覆盖两种写入模式、自动备份还原，写入时自动提权（Windows UAC）并刷新系统 DNS 缓存。

## 功能特性

### Profile 管理
- **本地 profile**：创建、重命名、删除、编辑 hosts 文本，一键启用/禁用/应用
- **远程 hosts**：订阅 `http(s)://` URL，创建时立即拉取；支持手动刷新与**定时自动刷新**（按 profile 独立设置间隔，30 秒轮询粒度）；启动时可自动刷新已启用的远程 hosts
- **多 profile 合并**：所有启用 profile 的条目合并为「托管块」写入系统 hosts，互不冲突

### 写入系统 hosts（桌面端）
- **追加模式（默认）**：条目写入系统 hosts 末尾的托管块，**保留系统原有条目**（如 `127.0.0.1 localhost`）
- **覆盖模式**：用当前启用 profile 的内容**完全替换**系统 hosts（切换前请自行确认，localhost 等条目需写入 profile 中保留）
- 托管块使用 `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<` 标记包裹，应用只增删该块，不触碰块外内容
- Windows 下自动提权写入（UAC），写入后自动刷新 DNS 缓存
- macOS / Linux 写入各自的标准 hosts 路径

### 安全与备份
- **每次写入前自动备份**当前系统 hosts，最多保留 50 份
- 手动备份 / 查看备份列表 / 一键还原任意备份

### DNS 代理（内置本地 DNS 服务器）
让 hosts 映射**无需写入系统 hosts** 也能生效，也是移动端的映射方案：

- **一键启停**：默认监听 `127.0.0.1:5353`（非特权端口，无需 root / 管理员）
- **映射直接应答**：启用 profile 中的域名直接返回映射 IP（`A` / `AAAA`），TTL 1 秒（保证开关配置立即生效）
- **其余查询转发上游**：未命中的域名转发给上游 DNS，上游应答原样回传（UDP，应答被截断时自动改用 TCP 重查）；本应用管理过的域名会被压掉应答 TTL，避免「关闭映射期间拿到的上游应答」被客户端长期缓存，导致重新开启后不生效
- **上游可配置**：默认自动探测系统 DNS（`/etc/resolv.conf`、`ipconfig`），也可在选项中填写（如 `223.5.5.5, 8.8.8.8`）
- **映射热更新**：启用/禁用 profile、编辑内容、删除、刷新远程 hosts、导入配置后立即同步，无需重启服务
- **运行状态可见**：监听地址、生效映射条数、命中映射次数、转发上游次数（每 2 秒刷新）
- **启动时自动运行**：可在选项中开启

> 桌面端可手动启动用于验证映射效果：`nslookup -port=5353 dev.local 127.0.0.1`

### 远程拉取（HTTP 代理）
- 支持为拉取远程 hosts 配置 **HTTP / HTTPS / SOCKS5** 代理

### 导入 / 导出
- 配置（profile 列表）支持 **JSON** 格式导出 / 导入
- 支持导出到文件、从文件导入

### 应用设置
- 界面语言：简体中文 / English
- 主题：浅色 / 深色
- 启动时隐藏到托盘、开机自启（桌面端）
- DNS 代理：监听端口、上游 DNS、启动时自动运行
- 自定义数据存储目录（可迁移）
- 平台信息查看：操作系统、桌面/移动端、hosts 路径（可一键打开所在文件夹）

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | Vue 3 (`<script setup>`) + TypeScript + Pinia + Element Plus |
| 桌面壳 | Tauri 2 |
| 后端 | Rust（tokio / hickory-proto / reqwest / serde / chrono / uuid 等） |

## 开发

```bash
# 安装依赖
npm install

# 以开发模式启动（桌面端）
npm run tauri dev

# 前端独立调试（纯浏览器，无 Tauri 环境，部分功能不可用）
npm run dev

# 运行 Rust 单元测试
cd src-tauri && cargo test
```

### 构建发布版

一键打当前系统能打的所有包（桌面端 + Android，macOS 上再加 iOS，各端互不影响）：

```bash
npm run build:all
```

也可以单独打某一端：

| 命令 | 说明 |
|---|---|
| `npm run build:check` | 体检打包环境（JDK / Android SDK / NDK / rustup target），不构建 |
| `npm run build:desktop` | 当前系统的桌面安装包（Windows `.exe`/`.msi`、macOS `.dmg`、Linux `.deb`/`.rpm`/`.AppImage`） |
| `npm run build:windows` / `build:macos` / `build:linux` | 显式指定系统；本机系统不匹配会直接报错，而不是抛原生工具链错误 |
| `npm run build:android` | Android APK（默认 arm64） |
| `npm run build:android:all` | 把全部 ABI 打进一个通用包 |
| `npm run build:android:split` | 每个 ABI 一个包（体积小） |
| `npm run build:android:debug` | 调试包（免签名、可调试） |
| `npm run build:android:aab` | Google Play 上架用的 AAB |
| `npm run build:ios` | iOS（需要 macOS + Xcode） |

产物统一收集到项目根目录，文件名带版本可追溯：`dist-desktop/`（安装包）、`dist-apk/`（`set-hosts-<abi>-release.apk`）、`dist-ios/`。

打包脚本在 `scripts/` 下，需要更细的控制时（选 ABI、拆分、只打某一端等）直接跑脚本：

```bash
# 只要 arm64 + armv7，并拆分成两个包
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi

# 只打 Android、跳过桌面端
node scripts/build-all.mjs --only android

# 各脚本的完整参数
node scripts/build-android.mjs --help
```

> **Windows PowerShell 注意**：`npm run xxx -- --flag value` 里带值的 `--flag` 会被 npm 吞掉（PowerShell 传参的老问题），需要传参时请用上面的 `node scripts/...` 写法，或用 `npm run build:android:all` 这类免传参的命令。

**Android 打包环境**：JDK 17+ 与 Android SDK（含 NDK）。脚本会自动探测常见安装位置，也可以用 `--java-home` / `--sdk` 显式指定，或设置 `JAVA_HOME` / `ANDROID_HOME`；缺 rustup target 时按 `npm run build:check` 的提示执行 `rustup target add aarch64-linux-android` 等命令补齐。

## 工作原理

### 桌面端：写入系统 hosts

1. 每个 profile 存储原始 hosts 文本（本地编辑或远程拉取）
2. 应用/启用/禁用 profile 时，后端收集**所有启用 profile** 的条目
3. 按设置的写入模式生成新 hosts 内容：
   - 追加：剥离旧托管块 → 在剩余内容末尾追加新托管块
   - 覆盖：整个文件仅保留新托管块
4. 提权写入前自动备份当前 hosts，写入后刷新 DNS 缓存

### DNS 代理：本地 DNS 服务器

1. 把所有启用 profile 的条目编译成 `域名 → IP` 映射表（同一域名以先出现的 profile 为准）
2. 访问配置变更时重新编译映射表并热更新，无需重启服务
3. 收到 DNS 查询时：
   - 命中映射 → 直接返回 `A` / `AAAA` 记录；映射存在但协议族不匹配 → 返回空应答，避免回落到真实 DNS
   - 未命中（或 `CNAME` / `MX` / `TXT` 等类型）→ 转发上游 DNS，应答原样回传
4. 移动端由 VPN 隧道把系统 DNS 指向 `127.0.0.1:<port>`，使映射对全系统生效

## 平台支持

| 平台 | 状态 |
|---|---|
| Windows / macOS / Linux | ✅ 完整支持（写入系统 hosts；DNS 代理可用于验证映射） |
| 移动端（Android / iOS） | 🚧 部分实现：内置 DNS 代理已完成（启停、映射、上游转发、状态统计）；把系统 DNS 交由该服务器接管的 **VPN 隧道仍需原生工程**（Android `VpnService` / iOS `NEPacketTunnelProvider`），待 `tauri android init` / Xcode 工程接入后生效 |

## 注意事项

- Windows 首次写入会弹出 UAC 提权确认，属正常现象
- 覆盖模式下系统原有 hosts 条目（含 `localhost`）会被移除，如需保留请将其写入 profile；误操作可从「备份与还原」恢复
- 远程 hosts 内容限制 8MB，拉取超时 15 秒
- DNS 代理默认监听 `127.0.0.1:5353`，未接入 VPN 隧道时仅本机（应用内）生效；桌面端若需全局生效请使用 hosts 写入
- DNS 代理上游单次查询超时 4 秒，失败返回 `SERVFAIL`（有本地命中时仍返回本地应答）

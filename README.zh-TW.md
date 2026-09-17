# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

一款跨平臺 hosts 設定管理工具（介面風格借鑒 SwitchHosts），基於 **Tauri 2 + Vue 3 + TypeScript + Rust** 構建：多設定檔（Profile）管理、遠端 hosts 訂閱、附加 / 覆蓋兩種寫入模式、自動備份與還原、遠端拉取支援代理；桌面端寫入時自動提權（Windows UAC）並重新整理系統 DNS 快取，行動端則透過系統 VPN 授權接管 DNS，讓對應生效。

## 功能特色

### 設定檔（Profile）管理

- **本機設定檔**：建立、重新命名、刪除、編輯 hosts 文字，一鍵啟用 / 停用 / 套用
- **遠端設定檔**：訂閱 `http(s)://` 網址，新增時立即拉取一次；名稱、URL、自動重新整理間隔隨時可改（修改 URL 會立即重新拉取，啟用狀態下還會重新套用到系統）
- **定時自動重新整理**：每個遠端設定檔可獨立設定間隔（永不 / 1 分鐘 / 5 分鐘 / 15 分鐘 / 1 小時 / 24 小時 / 7 天），背景以 30 秒粒度輪詢；也可開啟「啟動時自動重新整理」
- **多設定檔合併**：所有啟用設定檔的項目會合併寫入系統 hosts 的「託管區塊」，互不衝突

### 寫入系統 hosts（桌面端）

- **附加模式（預設）**：項目寫入系統 hosts 末尾的託管區塊，**保留系統原有項目**（如 `127.0.0.1 localhost`）
- **覆蓋模式**：以啟用設定檔的內容**完全取代**系統 hosts（切換前請確認 `localhost` 等項目已寫入設定檔）
- 託管區塊由 `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<` 標記包覆，套用時只增刪該區塊
- Windows 下自動提權寫入（UAC），寫入後自動重新整理 DNS 快取；macOS / Linux 寫入各自的標準 hosts 路徑

### 安全與備份

- **每次寫入系統 hosts 前自動備份**，最多保留最近 50 份
- 支援手動備份、檢視備份清單、一鍵還原任意備份

### 內建 DNS 代理（行動端對應方案）

桌面端直接改寫系統 hosts；行動端無法改寫系統 hosts，對應由「內建本機 DNS 伺服器 + 系統 VPN 接管 DNS」生效：

- **全自動**：設定檔開關由「關」切到「開」時，後端自動啟動本機 DNS 伺服器並申請系統 VPN 授權，介面上沒有相關設定項
- **預設監聽 `127.0.0.1:5353`**（非特權連接埠，無需 root / 管理員）；連接埠被佔用時自動退回系統隨機連接埠，確保服務一定能啟動
- **命中對應直接回應** `A` / `AAAA`，TTL 1 秒（確保開關設定立即生效）；**其餘查詢轉發上游** DNS（UDP，回應被截斷時自動改用 TCP 重查）
- **上游自動偵測**：自動偵測系統 DNS（`/etc/resolv.conf`、`ipconfig`）；DNS 代理沒有介面設定項，連接埠與上游均由後端自動決定
- **對應熱更新**：設定檔增刪改、啟停、重新整理遠端 hosts、匯入設定後立即同步，無需重啟

### 遠端拉取代理

- 拉取遠端 hosts 時可設定 **HTTP / HTTPS / SOCKS5** 代理，僅作用於本應用程式的拉取請求，不影響系統其他網路

### 匯入 / 匯出

- **JSON**：完整設定（所有設定檔與備份）匯出 / 匯入
- **Hosts 文字**：匯出目前設定檔的原始 hosts 文字，或把 hosts 文字匯入為新設定檔
- 支援匯出到檔案、從檔案匯入

### 應用程式設定

- **介面語言**：简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) —— 共 9 種，切換立即生效
- 主題：淺色 / 深色
- 啟動時隱藏到匣、開機自動啟動（桌面端）
- 寫入模式、遠端 hosts 拉取代理、啟動時自動重新整理遠端 hosts
- 平臺資訊檢視：作業系統、桌面 / 行動端、hosts 路徑（可一鍵開啟所在資料夾）
- 自訂資料儲存目錄（可遷移）

## 技術棧

| 層 | 技術 |
|---|---|
| 前端 | Vue 3（`<script setup>`）+ TypeScript + Pinia + Element Plus + Vite |
| 桌面 / 行動殼 | Tauri 2（tray-icon、dialog、shell、store、notification、autostart、single-instance 外掛） |
| 後端 | Rust：tokio、hickory-proto（DNS）、reqwest（遠端拉取；桌面用系統 TLS，行動端用 rustls）、serde、chrono、uuid |
| 行動端原生 | Android `VpnService`（`DnsVpnService.kt`，經 JNI 與 Rust 互動） |

## 開發

```bash
npm install                  # 安裝依賴
npm run tauri dev            # 桌面端開發模式
npm run dev                  # 僅前端除錯（無 Tauri 環境，部分功能不可用）
cd src-tauri && cargo test   # Rust 單元測試
```

行動端開發：

```bash
npm run tauri android dev    # Android（需 JDK 17+ 與 Android SDK / NDK）
npm run tauri ios dev        # iOS（需 macOS + Xcode）
```

## 建置與發行

一鍵打包目前系統能打的所有端（桌面端 + Android，macOS 上再加 iOS，各端互不影響）：

```bash
npm run build:all
```

| 指令 | 說明 |
|---|---|
| `npm run build:check` | 體檢打包環境（JDK / Android SDK / NDK / rustup target），不建置 |
| `npm run build:desktop` | 目前系統桌面安裝包（Windows `.exe` / `.msi`、macOS `.dmg`、Linux `.deb` / `.rpm` / `.AppImage`） |
| `npm run build:windows` / `build:macos` / `build:linux` | 明確指定系統；與本機不符時直接報錯 |
| `npm run build:android` | Android APK（預設 arm64） |
| `npm run build:android:all` | 把全部 ABI 打進一個通用包 |
| `npm run build:android:split` | 每個 ABI 一個包（體積小） |
| `npm run build:android:debug` | 除錯包（免簽章、可除錯） |
| `npm run build:android:aab` | Google Play 上架用的 AAB |
| `npm run build:ios` | iOS（需 macOS + Xcode） |

產物統一收集到專案根目錄：`dist-desktop/`、`dist-apk/`、`dist-ios/`。

需要更細的控制時直接跑指令稿（Windows PowerShell 下 `npm run xxx -- --flag value` 裡帶值的參數會被吞掉）：

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Android 打包環境：JDK 17+ 與 Android SDK（含 NDK）。指令稿會自動偵測常見安裝位置，也可用 `--java-home` / `--sdk` 指定，或設定 `JAVA_HOME` / `ANDROID_HOME`；缺少 rustup target 時依 `npm run build:check` 的提示執行 `rustup target add aarch64-linux-android` 等指令補齊。

### 版本管理

版本號以 `package.json` 為唯一來源，`scripts/bump-version.mjs` 會同步 **8 處**：`package.json`、`package-lock.json`、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`、Android `tauri.properties`（`versionName` / `versionCode`）、Android 內嵌 `tauri.conf.json`、iOS `project.pbxproj`。

| 指令 | 說明 |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | 遞增版本 → 同步全部檔案 → 提交 → 打 `v*` 附註標籤 |
| `npm run version:sync` | 不遞增，僅把其他檔案同步為 `package.json` 目前版本 |
| `npm run version:check` | 校驗各處版本號是否一致（不一致時結束碼非 0） |
| `npm run push` | `git push --follow-tags`，同時推送程式碼與標籤 |

關於頁顯示的版本在建置時從 `package.json` 注入，打包各端時始終為目前最新版本的版號。

## 運作原理

### 桌面端：寫入系統 hosts

1. 每個設定檔保存原始 hosts 文字（本機編輯或遠端拉取）
2. 啟用 / 停用 / 套用時，後端收集**所有啟用設定檔**的項目
3. 依寫入模式產生新內容：附加 = 移除舊託管區塊後附加新區塊；覆蓋 = 整個檔案僅保留新託管區塊
4. 提權寫入前自動備份目前 hosts，寫入後重新整理 DNS 快取

### 行動端：內建 DNS 代理 + VPN 接管

1. 把所有啟用設定檔的項目編譯成 `網域 → IP` 對應表（同一網域以先出現的設定檔為準）
2. 設定變更時重新編譯對應表並熱更新，無需重啟服務
3. 收到 DNS 查詢時：命中對應 → 直接回傳 `A` / `AAAA`（協定族不符回傳空回應，避免回落到真實 DNS）；未命中或 `CNAME` / `MX` 等類型 → 轉發上游 DNS 並原樣回傳
4. Android 由 `DnsVpnService` 把系統 DNS 指向本機伺服器，對應對全系統生效

## 平臺支援

| 平臺 | 狀態 |
|---|---|
| Windows / macOS / Linux | ✅ 完整支援：寫入系統 hosts、自動備份、遠端訂閱、匯入匯出 |
| Android | ✅ 支援：內建 DNS 代理 + `VpnService` 接管系統 DNS（首次開啟設定檔會彈出一次系統 VPN 授權，系統依套件名稱永久記住） |
| iOS | 🚧 部分支援：DNS 代理與前端已完成，`Network Extension` 隧道尚未接入（需付費開發者帳號與 entitlement），隧道生效前對應不會作用於全系統 |

## 目錄結構

```
src/            前端（Vue 3 + TS）：頁面、元件、Pinia store、i18n（9 種語言）
src-tauri/      Rust 後端：指令層、hosts 解析、備份、DNS 代理、行動端原生橋接
scripts/        打包指令稿與版本管理指令稿
dist-desktop/   桌面端安裝包產物
dist-apk/       Android 產物
```

## 注意事項

- Windows 首次寫入系統 hosts 會彈出 UAC 提權確認，屬正常現象
- 覆蓋模式會移除系統原有 hosts 項目（含 `localhost`），如需保留請寫入設定檔中；誤操作可從「備份與還原」恢復
- 遠端 hosts 單次內容上限 8MB、拉取逾時 15 秒；內容應為標準 hosts 格式純文字（每行 `IP 網域`），不支援 JSON
- Android 首次開啟設定檔需授予 VPN 授權；拒絕後開關會自動回滾，再次開啟會重新申請
- DNS 代理上游單次查詢逾時 4 秒，失敗回傳 `SERVFAIL`

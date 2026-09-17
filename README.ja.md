# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

クロスプラットフォーム対応の hosts 管理ツール（UI は SwitchHosts を参考）です。**Tauri 2 + Vue 3 + TypeScript + Rust** で構築されています：複数プロファイル管理、リモート hosts 購読、追記 / 上書きの 2 つの書き込みモード、自動バックアップと復元、リモート取得用プロキシに対応。デスクトップでは権限昇格（Windows UAC）でシステム hosts に書き込み DNS キャッシュを更新し、モバイルではシステム VPN 権限で DNS を引き受けてマッピングを反映させます。

## 主な機能

### プロファイル管理

- **ローカルプロファイル**：hosts テキストの作成、名前変更、削除、編集、ワンクリックでの有効化 / 無効化 / 適用
- **リモートプロファイル**：`http(s)://` の URL を購読し、追加時に即座に取得。名前・URL・自動更新間隔はいつでも変更可能（URL を変更すると即再取得し、有効時は再適用されます）
- **自動更新**：リモートプロファイルごとに間隔を設定（なし / 1 分 / 5 分 / 15 分 / 1 時間 / 24 時間 / 7 日）。バックグラウンドで 30 秒粒度でポーリングし、起動時の自動更新も有効にできます
- **マージ**：有効な全プロファイルのエントリをまとめてシステム hosts の「管理ブロック」に書き込みます

### システム hosts への書き込み（デスクトップ）

- **追記モード（既定）**：システム hosts 末尾の管理ブロックに追記し、**既存のエントリ（`127.0.0.1 localhost` など）を保持**します
- **上書きモード**：有効なプロファイルの内容でシステム hosts を**完全に置換**します（`localhost` などをプロファイルに含めてください）
- 管理ブロックは `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<` で囲まれ、適用時はこのブロックのみを追加・削除します
- Windows では自動的に権限昇格（UAC）して書き込み、書き込み後に DNS キャッシュを更新。macOS / Linux はそれぞれの標準パスに書き込みます

### 安全性とバックアップ

- **システム hosts への書き込み前に毎回自動バックアップ**（最新 50 件を保持）
- 手動バックアップ、バックアップ一覧、任意バックアップのワンクリック復元

### 内蔵 DNS プロキシ（モバイルのマッピング方式）

デスクトップはシステム hosts を直接書き換えますが、モバイルでは「内蔵ローカル DNS サーバー + システム VPN による DNS 引き受け」で反映します：

- **全自動**：プロファイルのスイッチをオフからオンにすると、バックエンドがローカル DNS サーバーを起動しシステム VPN 権限を要求します。UI に関連設定はありません
- **既定の待ち受けは `127.0.0.1:5353`**（非特権ポート、root / 管理者権限不要）。使用中の場合はシステムのランダムポートへ自動的にフォールバックし、確実に起動します
- **マッピング命中時は `A` / `AAAA` を直接応答**（TTL 1 秒で即時反映）。**それ以外は上流 DNS へ転送**（UDP。応答が切り詰められた場合は TCP で再試行）
- **上流は自動検出**：システム DNS を自動検出します（`/etc/resolv.conf`、`ipconfig`）。DNS プロキシに UI 設定はなく、ポートと上流はバックエンドが自動決定します
- **ホットリロード**：編集・スイッチ・リモート更新・インポート後すぐにマッピングを同期。再起動は不要です

### リモート取得用プロキシ

- リモート hosts の取得に **HTTP / HTTPS / SOCKS5** プロキシを設定できます。本アプリの取得リクエストのみに適用されます

### インポート / エクスポート

- **JSON**：全設定（全プロファイルとバックアップ）をエクスポート / インポート
- **Hosts テキスト**：現在のプロファイルの生の hosts テキストをエクスポート、または hosts テキストを新規プロファイルとしてインポート
- ファイルへのエクスポート / ファイルからのインポートに対応

### アプリ設定

- **UI 言語**：简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) —— 全 9 言語、切り替えは即時反映
- テーマ：ライト / ダーク
- 起動時にトレイに隠す、ログイン時に自動起動（デスクトップ）
- 書き込みモード、リモート hosts 取得用プロキシ、起動時の自動更新
- プラットフォーム情報：OS、デスクトップ / モバイル、hosts パス（フォルダをワンクリックで開く）
- データ保存ディレクトリの変更（移行可能）

## 技術スタック

| 層 | 技術 |
|---|---|
| フロントエンド | Vue 3（`<script setup>`）+ TypeScript + Pinia + Element Plus + Vite |
| シェル | Tauri 2（tray-icon、dialog、shell、store、notification、autostart、single-instance プラグイン） |
| バックエンド | Rust：tokio、hickory-proto（DNS）、reqwest（リモート取得。デスクトップはシステム TLS、モバイルは rustls）、serde、chrono、uuid |
| モバイルネイティブ | Android `VpnService`（`DnsVpnService.kt`、JNI 経由で Rust と連携） |

## 開発

```bash
npm install                  # 依存関係のインストール
npm run tauri dev            # デスクトップ開発モード
npm run dev                  # フロントエンドのみ（Tauri 環境なし、一部機能は利用不可）
cd src-tauri && cargo test   # Rust のユニットテスト
```

モバイル開発：

```bash
npm run tauri android dev    # Android（JDK 17+ と Android SDK / NDK が必要）
npm run tauri ios dev        # iOS（macOS + Xcode が必要）
```

## ビルドとリリース

現在のマシンでビルド可能なすべてのターゲットをまとめてビルドします（デスクトップ + Android、macOS ではさらに iOS。各ターゲットは独立）：

```bash
npm run build:all
```

| コマンド | 説明 |
|---|---|
| `npm run build:check` | ビルド環境を点検（JDK / Android SDK / NDK / rustup target）。ビルドはしません |
| `npm run build:desktop` | 現在の OS 向けデスクトップインストーラ（Windows `.exe` / `.msi`、macOS `.dmg`、Linux `.deb` / `.rpm` / `.AppImage`） |
| `npm run build:windows` / `build:macos` / `build:linux` | ターゲット OS を明示指定。環境不一致なら即エラー |
| `npm run build:android` | Android APK（既定は arm64） |
| `npm run build:android:all` | 全 ABI を 1 つのユニバーサルパッケージに |
| `npm run build:android:split` | ABI ごとに分割（サイズ小） |
| `npm run build:android:debug` | デバッグビルド（未署名、デバッグ可能） |
| `npm run build:android:aab` | Google Play 提出用 AAB |
| `npm run build:ios` | iOS（macOS + Xcode が必要） |

成果物はプロジェクトルートに集約されます：`dist-desktop/`、`dist-apk/`、`dist-ios/`。

細かく制御したい場合はスクリプトを直接実行してください（Windows PowerShell では `npm run xxx -- --flag value` の値付きフラグが失われます）：

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Android のビルド環境：JDK 17+ と Android SDK（NDK を含む）。スクリプトは一般的なインストール先を自動検出します。`--java-home` / `--sdk` や `JAVA_HOME` / `ANDROID_HOME` でも指定可能。rustup target が不足している場合は `npm run build:check` の案内に従い `rustup target add aarch64-linux-android` などを実行してください。

### バージョン管理

バージョンは `package.json` を唯一の情報源とし、`scripts/bump-version.mjs` が **8 か所**を同期します：`package.json`、`package-lock.json`、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`、Android `tauri.properties`（`versionName` / `versionCode`）、Android 同梱の `tauri.conf.json`、iOS `project.pbxproj`。

| コマンド | 説明 |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | バージョンを上げる → 全ファイルを同期 → コミット → `v*` 注釈タグを作成 |
| `npm run version:sync` | バージョンは上げず、他のファイルを現在の `package.json` のバージョンに同期 |
| `npm run version:check` | 各所のバージョン一致を検証（不一致なら終了コード 0 以外） |
| `npm run push` | `git push --follow-tags`：コミットとタグをまとめてプッシュ |

概要ページのバージョンはビルド時に `package.json` から注入されるため、どのターゲットでも常に最新のバージョン番号になります。

## 仕組み

### デスクトップ：システム hosts への書き込み

1. 各プロファイルは生の hosts テキストを保持（ローカル編集またはリモート取得）
2. 有効化 / 無効化 / 適用時に、**有効な全プロファイル**のエントリを収集
3. 書き込みモードに応じて内容を生成：追記 = 旧管理ブロックを除去して新ブロックを追記、上書き = 新管理ブロックのみを残す
4. 権限昇格による書き込み前に現在の hosts をバックアップし、書き込み後に DNS キャッシュを更新

### モバイル：内蔵 DNS プロキシ + VPN

1. 有効な全プロファイルを `ドメイン → IP` のマッピング表にコンパイル（重複は先に出現したプロファイルを優先）
2. 設定変更時に再コンパイルしてホット更新。再起動は不要
3. クエリ時：マッピング命中 → `A` / `AAAA` を直接応答（ファミリ不一致なら空応答を返し実 DNS へのフォールバックを防止）。未命中や `CNAME` / `MX` など → 上流へ転送してそのまま返す
4. Android では `DnsVpnService` がシステム DNS をローカルサーバーへ向け、マッピングがシステム全体で有効になります

## プラットフォーム対応

| プラットフォーム | 状態 |
|---|---|
| Windows / macOS / Linux | ✅ 完全対応：システム hosts への書き込み、自動バックアップ、リモート購読、インポート / エクスポート |
| Android | ✅ 対応：内蔵 DNS プロキシ + `VpnService` による DNS 引き受け（初回有効化時に一度だけシステム VPN 権限ダイアログが表示され、パッケージ単位で記憶されます） |
| iOS | 🚧 一部対応：DNS プロキシとフロントエンドは完成、`Network Extension` トンネルは未接続（有料開発者アカウントと entitlement が必要）。トンネルが有効になるまでマッピングはシステム全体に適用されません |

## ディレクトリ構成

```
src/            フロントエンド（Vue 3 + TS）：画面、コンポーネント、Pinia store、i18n（9 言語）
src-tauri/      Rust バックエンド：コマンド層、hosts 解析、バックアップ、DNS プロキシ、モバイルネイティブ橋渡し
scripts/        ビルドスクリプトとバージョン管理スクリプト
dist-desktop/   デスクトップインストーラの成果物
dist-apk/       Android の成果物
```

## 注意事項

- Windows での初回書き込み時に UAC の権限昇格ダイアログが表示されますが、正常な動作です
- 上書きモードは既存の hosts エントリ（`localhost` を含む）を削除します。残したい場合はプロファイルに記載してください。誤操作は「バックアップと復元」から復元できます
- リモート hosts の上限は 8MB、取得タイムアウトは 15 秒。内容は標準 hosts 形式のプレーンテキスト（1 行に `IP ドメイン`）で、JSON は非対応です
- Android で初めてプロファイルを有効にする際は VPN 権限の許可が必要です。拒否するとスイッチは自動的に戻り、次回有効化時に再度要求されます
- DNS プロキシの上流クエリのタイムアウトは 4 秒で、失敗時は `SERVFAIL` を返します

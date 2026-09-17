# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

크로스 플랫폼 hosts 관리 도구입니다(SwitchHosts 스타일의 UI). **Tauri 2 + Vue 3 + TypeScript + Rust**로 만들었습니다: 여러 프로필 관리, 원격 hosts 구독, 추가 / 덮어쓰기 두 가지 쓰기 모드, 자동 백업 및 복원, 원격 가져오기용 프록시 지원. 데스크톱에서는 권한 상승(Windows UAC)으로 시스템 hosts에 쓰고 DNS 캐시를 비우며, 모바일에서는 시스템 VPN 권한으로 DNS를 넘겨받아 매핑을 적용합니다.

## 주요 기능

### 프로필 관리

- **로컬 프로필**: hosts 텍스트 생성, 이름 변경, 삭제, 편집, 한 번의 클릭으로 활성화 / 비활성화 / 적용
- **원격 프로필**: `http(s)://` URL을 구독하고 추가 시 즉시 가져옵니다. 이름·URL·자동 새로 고침 주기는 언제든 수정 가능(URL을 바꾸면 즉시 다시 가져오고, 활성 상태면 다시 적용)
- **자동 새로 고침**: 원격 프로필마다 주기 설정(사용 안 함 / 1분 / 5분 / 15분 / 1시간 / 24시간 / 7일), 백그라운드에서 30초 단위로 폴링. 시작 시 자동 새로 고침도 켤 수 있습니다
- **병합**: 활성화된 모든 프로필의 항목을 시스템 hosts의 「관리 블록」에 합쳐 씁니다

### 시스템 hosts 쓰기(데스크톱)

- **추가 모드(기본)**: 시스템 hosts 끝의 관리 블록에 추가하며 **기존 항목(`127.0.0.1 localhost` 등)을 유지**합니다
- **덮어쓰기 모드**: 활성 프로필 내용으로 시스템 hosts를 **완전히 교체**합니다(`localhost` 등을 프로필에 넣어 두세요)
- 관리 블록은 `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<`로 감싸며, 적용 시 이 블록만 더하고 뺍니다
- Windows에서는 자동으로 권한 상승(UAC)하여 쓰고, 쓴 뒤 DNS 캐시를 비웁니다. macOS / Linux는 각자의 표준 hosts 경로에 씁니다

### 안전성과 백업

- **시스템 hosts에 쓰기 전마다 자동 백업**(최근 50개 보관)
- 수동 백업, 백업 목록 확인, 원하는 백업 한 번에 복원

### 내장 DNS 프록시(모바일 매핑 방식)

데스크톱은 시스템 hosts를 직접 고치지만, 모바일은 「내장 로컬 DNS 서버 + 시스템 VPN을 통한 DNS 넘겨받기」로 적용합니다:

- **완전 자동**: 프로필 스위치를 껐다 켜면 백엔드가 로컬 DNS 서버를 띄우고 시스템 VPN 권한을 요청합니다. UI에 관련 설정은 없습니다
- **기본 수신 주소는 `127.0.0.1:5353`**(비특권 포트, root / 관리자 불필요). 포트가 사용 중이면 시스템 임의 포트로 자동 대체하여 반드시 기동합니다
- **매핑에 맞으면 `A` / `AAAA`로 바로 응답**(TTL 1초로 즉시 반영). **그 밖의 질의는 상위 DNS로 전달**(UDP, 응답이 잘리면 TCP로 재시도)
- **상위 DNS 자동 감지**: 시스템 DNS를 자동 감지합니다(`/etc/resolv.conf`, `ipconfig`). DNS 프록시에는 UI 설정이 없으며 포트와 상위는 백엔드가 자동으로 결정합니다
- **즉시 반영**: 편집·스위치·원격 새로 고침·가져오기 후 매핑이 바로 동기화되며 재시작이 필요 없습니다

### 원격 가져오기용 프록시

- 원격 hosts를 가져올 때 **HTTP / HTTPS / SOCKS5** 프록시를 설정할 수 있으며, 이 앱의 가져오기 요청에만 적용됩니다

### 가져오기 / 내보내기

- **JSON**: 전체 설정(모든 프로필과 백업) 내보내기 / 가져오기
- **Hosts 텍스트**: 현재 프로필의 원본 hosts 텍스트 내보내기, 또는 hosts 텍스트를 새 프로필로 가져오기
- 파일로 내보내기 / 파일에서 가져오기 지원

### 앱 설정

- **UI 언어**: 简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) —— 총 9종, 전환 즉시 반영
- 테마: 라이트 / 다크
- 시작 시 트레이로 숨기기, 로그인 시 자동 실행(데스크톱)
- 쓰기 모드, 원격 hosts 가져오기용 프록시, 시작 시 원격 hosts 자동 새로 고침
- 플랫폼 정보: 운영체제, 데스크톱 / 모바일, hosts 경로(폴더 바로 열기)
- 데이터 저장 폴더 변경(이전 가능)

## 기술 스택

| 계층 | 기술 |
|---|---|
| 프론트엔드 | Vue 3(`<script setup>`) + TypeScript + Pinia + Element Plus + Vite |
| 셸 | Tauri 2(tray-icon, dialog, shell, store, notification, autostart, single-instance 플러그인) |
| 백엔드 | Rust: tokio, hickory-proto(DNS), reqwest(원격 가져오기. 데스크톱은 시스템 TLS, 모바일은 rustls), serde, chrono, uuid |
| 모바일 네이티브 | Android `VpnService`(`DnsVpnService.kt`, JNI로 Rust와 연동) |

## 개발

```bash
npm install                  # 의존성 설치
npm run tauri dev            # 데스크톱 개발 모드
npm run dev                  # 프론트엔드만(Tauri 환경 없음, 일부 기능 미동작)
cd src-tauri && cargo test   # Rust 단위 테스트
```

모바일 개발:

```bash
npm run tauri android dev    # Android(JDK 17+ 및 Android SDK / NDK 필요)
npm run tauri ios dev        # iOS(macOS + Xcode 필요)
```

## 빌드와 배포

현재 머신에서 빌드 가능한 모든 대상을 한 번에 빌드합니다(데스크톱 + Android, macOS에서는 iOS 추가. 각 대상은 독립적):

```bash
npm run build:all
```

| 명령 | 설명 |
|---|---|
| `npm run build:check` | 빌드 환경 점검(JDK / Android SDK / NDK / rustup target). 빌드는 하지 않음 |
| `npm run build:desktop` | 현재 OS용 데스크톱 설치 파일(Windows `.exe` / `.msi`, macOS `.dmg`, Linux `.deb` / `.rpm` / `.AppImage`) |
| `npm run build:windows` / `build:macos` / `build:linux` | 대상 OS 명시. 환경이 다르면 바로 오류 |
| `npm run build:android` | Android APK(기본 arm64) |
| `npm run build:android:all` | 모든 ABI를 하나의 범용 패키지로 |
| `npm run build:android:split` | ABI별 개별 패키지(용량 작음) |
| `npm run build:android:debug` | 디버그 빌드(서명 없음, 디버그 가능) |
| `npm run build:android:aab` | Google Play 제출용 AAB |
| `npm run build:ios` | iOS(macOS + Xcode 필요) |

결과물은 프로젝트 루트에 모입니다: `dist-desktop/`, `dist-apk/`, `dist-ios/`.

세밀한 제어가 필요하면 스크립트를 직접 실행하세요(Windows PowerShell에서는 `npm run xxx -- --flag value`의 값 있는 플래그가 사라집니다):

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Android 빌드 환경: JDK 17+ 및 Android SDK(NDK 포함). 스크립트가 흔한 설치 위치를 자동으로 찾으며, `--java-home` / `--sdk` 또는 `JAVA_HOME` / `ANDROID_HOME`으로 지정할 수도 있습니다. rustup target이 없으면 `npm run build:check` 안내대로 `rustup target add aarch64-linux-android` 등을 실행하세요.

### 버전 관리

버전은 `package.json`이 단일 출처이며, `scripts/bump-version.mjs`가 **8곳**을 동기화합니다: `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, Android `tauri.properties`(`versionName` / `versionCode`), Android에 포함된 `tauri.conf.json`, iOS `project.pbxproj`.

| 명령 | 설명 |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | 버전 올리기 → 모든 파일 동기화 → 커밋 → `v*` 주석 태그 생성 |
| `npm run version:sync` | 올리지 않고 다른 파일을 현재 `package.json` 버전으로 동기화 |
| `npm run version:check` | 모든 곳의 버전이 같은지 검증(불일치 시 종료 코드 0 아님) |
| `npm run push` | `git push --follow-tags`: 커밋과 태그를 함께 푸시 |

정보 화면의 버전은 빌드 시 `package.json`에서 주입되므로 어떤 대상을 빌드해도 항상 최신 버전 번호가 표시됩니다.

## 동작 방식

### 데스크톱: 시스템 hosts 쓰기

1. 각 프로필은 원본 hosts 텍스트를 보관(로컬 편집 또는 원격 가져오기)
2. 활성화 / 비활성화 / 적용 시 **활성화된 모든 프로필**의 항목을 수집
3. 쓰기 모드에 따라 내용 생성: 추가 = 이전 관리 블록 제거 후 새 블록 추가, 덮어쓰기 = 새 관리 블록만 유지
4. 권한 상승으로 쓰기 전에 현재 hosts를 백업하고, 쓴 뒤 DNS 캐시를 비움

### 모바일: 내장 DNS 프록시 + VPN

1. 활성화된 모든 프로필을 `도메인 → IP` 매핑 표로 컴파일(중복 도메인은 먼저 나온 프로필 우선)
2. 설정이 바뀌면 다시 컴파일하여 즉시 반영. 재시작 불필요
3. 질의 처리: 매핑에 맞으면 `A` / `AAAA`로 바로 응답(패밀리가 다르면 빈 응답을 반환해 실제 DNS로 넘어가지 않게 함). 미해당 또는 `CNAME` / `MX` 등은 상위로 전달해 그대로 반환
4. Android에서는 `DnsVpnService`가 시스템 DNS를 로컬 서버로 향하게 하여 매핑이 시스템 전체에 적용됩니다

## 플랫폼 지원

| 플랫폼 | 상태 |
|---|---|
| Windows / macOS / Linux | ✅ 완전 지원: 시스템 hosts 쓰기, 자동 백업, 원격 구독, 가져오기 / 내보내기 |
| Android | ✅ 지원: 내장 DNS 프록시 + `VpnService`로 시스템 DNS 넘겨받기(처음 프로필을 켤 때 시스템 VPN 권한 창이 한 번 뜨며 패키지 단위로 기억됨) |
| iOS | 🚧 부분 지원: DNS 프록시와 프론트엔드는 완료, `Network Extension` 터널은 미연결(유료 개발자 계정과 entitlement 필요). 터널이 연결되기 전에는 매핑이 시스템 전체에 적용되지 않습니다 |

## 디렉터리 구조

```
src/            프론트엔드(Vue 3 + TS): 화면, 컴포넌트, Pinia store, i18n(9개 언어)
src-tauri/      Rust 백엔드: 커맨드 계층, hosts 파싱, 백업, DNS 프록시, 모바일 네이티브 브리지
scripts/        빌드 스크립트와 버전 관리 스크립트
dist-desktop/   데스크톱 설치 파일 결과물
dist-apk/       Android 결과물
```

## 주의사항

- Windows에서 처음 쓸 때 UAC 권한 상승 창이 뜨는 것은 정상입니다
- 덮어쓰기 모드는 기존 hosts 항목(`localhost` 포함)을 제거합니다. 유지하려면 프로필에 넣으세요. 실수했다면 「백업 및 복원」에서 복구할 수 있습니다
- 원격 hosts는 8MB 제한, 15초 타임아웃. 내용은 표준 hosts 형식의 일반 텍스트(한 줄에 `IP 도메인`)여야 하며 JSON은 지원하지 않습니다
- Android에서 처음 프로필을 켤 때는 VPN 권한 허용이 필요합니다. 거부하면 스위치가 자동으로 되돌아가고 다음에 켤 때 다시 요청합니다
- DNS 프록시의 상위 질의 타임아웃은 4초이며 실패 시 `SERVFAIL`을 반환합니다

# iOS 隧道接入说明（Network Extension）

Android 由 `gen/android/.../DnsVpnService.kt` + `src-tauri/src/mobile/android.rs`（JNI）接管系统 DNS；
iOS 沙盒禁止 App 直接读写系统 hosts，必须使用 **Network Extension** 才能把 DNS 交给本应用的
Rust 本地 DNS 服务器（`src-tauri/src/dns_proxy`，默认监听 `127.0.0.1:5353`）。

本目录提供 iOS 侧的原生实现源码，但**接入必须在 macOS + Xcode 完成**，详见下方步骤。

## 文件

| 文件 | 归属 Target | 作用 |
|---|---|---|
| `DNSProxyProvider.swift` | DNS Proxy 扩展 Target | `NEDNSProxyProvider`：把系统 DNS 查询转发到 `127.0.0.1:<port>` |
| `DNSProxyController.swift` | 宿主 App Target | `NEDNSProxyManager`：启停 DNS 代理、查询接管状态（对应 Android `DnsVpn.kt`） |

## 接入步骤

1. **生成 iOS 工程**（需 macOS + Xcode）：

   ```bash
   npm run tauri ios init
   ```

   之后会生成 `src-tauri/gen/apple/`（当前仓库里没有，故 `capabilities/mobile-ios.json` 引用的
   `gen/schemas/ios-schema.json` 也不存在）。

2. **新建扩展 Target**：Xcode → File → New → Target → *Network Extension*，Provider Type 选
   **DNS Proxy**，Product Bundle Identifier 填 `com.sethosts.app.DNSProxyExtension`
   （与 `DNSProxyController.providerBundleIdentifier` 一致）。把 `DNSProxyProvider.swift`
   加入该 Target；`DNSProxyController.swift` 加入宿主 App Target。

3. **申请授权**：宿主 App 与扩展 Target 都要开启
   `com.apple.developer.networking.networkextension = dns-proxy`。
   该能力需要**付费 Apple 开发者账号**向 Apple 申请，个人免费账号无法使用。

4. **Rust 侧接线**（`src-tauri/src/mobile/ios.rs` 目前是占位实现，两个函数直接返回 `Err`）：
   Swift 无法从 Rust 直接调用，用 `swift-rs` 桥接即可：

   ```toml
   # src-tauri/Cargo.toml
   [target.'cfg(target_os = "ios")'.dependencies]
   swift-rs = "1"
   ```

   Swift 侧暴露 C 符号（新建一个 `SetHostsDNSBridge.swift` 加入宿主 Target）：

   ```swift
   @_cdecl("set_hosts_dns_proxy_start")
   func setHostsDNSProxyStart(port: Int32, callback: @escaping (Int32) -> Void) {
       Task {
           do { try await DNSProxyController.start(port: UInt16(port)); callback(0) }
           catch { callback(-1) }
       }
   }

   @_cdecl("set_hosts_dns_proxy_stop")
   func setHostsDNSProxyStop() {
       Task { await DNSProxyController.stop() }
   }
   ```

   然后把 `mobile/ios.rs` 的 `start_tunnel` / `stop_tunnel` 改为调用这两个符号，
   `is_tunnel_active` 改为调用 `DNSProxyController.isActive()`。

## 当前状态

- Rust 本地 DNS 服务器、映射编译与热更新、前端交互：**已完成**（与 Android 共用）
- 系统 DNS 接管（隧道）：**未实现**，`mobile/ios.rs` 返回 `Err`
- 后果：`sync_mobile_tunnel` 会失败并回滚配置开关，即 iOS 上开关目前无法打开。
  这是刻意保留的行为——开关显示「已启用」而映射实际未生效，比打不开更糟。
- iOS 亦无法读写系统 hosts、无备份还原 / 导入导出 / DNS 缓存刷新（与 Android 一致的限制）

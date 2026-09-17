package com.sethosts.app

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.ScriptHandler
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature

class MainActivity : TauriActivity() {
  private var webView: WebView? = null
  private var insetsScript: ScriptHandler? = null
  private var insetTop = 0f
  private var insetBottom = 0f

  /**
   * 关闭 wry 默认的返回处理（WebView 无历史记录时返回手势会直接退出应用）。
   * 返回手势 / 返回键改由 [onWebViewCreate] 中注册的回调转发给前端
   * `window.__onAndroidBack()` 统一处理：关闭子抽屉 → 关闭设置菜单 → 双击退出。
   */
  override val handleBackNavigation: Boolean = false

  override fun onCreate(savedInstanceState: Bundle?) {
    // Android 15+ 强制 edge-to-edge：WebView 会一直绘制到状态栏 / 导航栏下方。
    // 这里保持全屏铺满，安全区交给前端 CSS（这样状态栏区域的背景色才能跟随应用内主题）。
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    listenSafeAreaInsets()
    // 把 JavaVM 与 Activity 引用交给 Rust，供 DNS 隧道（VpnService）启停使用
    runCatching { DnsVpn.nativeInit(this) }
  }

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
    applySafeAreaInsets()
    // 返回手势 / 返回键 → 前端统一处理（关闭抽屉 / 关闭设置菜单 / 双击退出）。
    // 前端确认退出后会走 Rust 的 exit_app 命令回调 [exitApp]。
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        webView.evaluateJavascript(
          "window.__onAndroidBack ? window.__onAndroidBack() : null"
        ) { _ -> }
      }
    })
  }

  override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
    super.onActivityResult(requestCode, resultCode, data)
    // VPN 授权结果
    DnsVpn.onActivityResult(this, requestCode, resultCode)
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    // 回到前台时重新派发一次，确保旋转 / 分屏后安全区尺寸同步
    if (hasFocus) {
      contentView()?.let { ViewCompat.requestApplyInsets(it) }
    }
  }

  /**
   * 以下三个方法由 Rust 通过 JNI 调用，见 `src-tauri/src/mobile/android.rs`：
   * DNS 代理启动/停止时同步建立或关闭 VpnService 隧道。
   */
  @Suppress("unused")
  fun startDnsVpn(port: Int) {
    DnsVpn.start(this, port)
  }

  /**
   * 自动接管系统 DNS（应用启动 / 回到前台时由 Rust 调用）：
   * 已授权时完全静默，首次运行自动弹一次系统 VPN 授权，用户拒绝过就不再打扰。
   */
  @Suppress("unused")
  fun autoStartDnsVpn(port: Int) {
    DnsVpn.autoStart(this, port)
  }

  @Suppress("unused")
  fun stopDnsVpn() {
    DnsVpn.stop(this)
  }

  @Suppress("unused")
  fun isDnsVpnRunning(): Boolean = DnsVpn.isRunning()

  /**
   * 退出应用（前端「再按一次退出」确认后由 Rust 通过 JNI 调用，见
   * `src-tauri/src/mobile/android.rs`）：停止 DNS 隧道 → 结束 Activity → 终止进程。
   *
   * 必须显式杀进程：Tauri 运行时不会随 Activity 销毁而退出，进程若残留，
   * 下次启动无法重新初始化（表现为白屏）。
   */
  @Suppress("unused")
  fun exitApp() {
    runOnUiThread {
      // 先停掉 VPN 隧道，让系统钥匙图标立即消失；进程终止后系统也会兜底回收
      runCatching { stopService(Intent(this, DnsVpnService::class.java)) }
      finish()
      android.os.Process.killProcess(android.os.Process.myPid())
    }
  }

  /**
   * 读取 / 清空隧道诊断日志（由 Rust 通过 JNI 调用，见 `src-tauri/src/mobile/android.rs`）。
   *
   * 隧道跑在 Service 里、日志只进 logcat，用户界面和 Rust 都拿不到；
   * 这里把它取出来交给前端「诊断日志」页展示与复制，真机排查不必再依赖 adb。
   */
  @Suppress("unused")
  fun vpnDiagnostics(): String = DnsVpn.diagnostics(this)

  @Suppress("unused")
  fun clearVpnDiagnostics() {
    DnsVpn.clearDiagnostics(this)
  }

  private fun contentView(): View? = findViewById(android.R.id.content)

  /** 监听系统栏尺寸变化，把结果同步给前端 */
  private fun listenSafeAreaInsets() {
    val root = contentView() ?: return
    ViewCompat.setOnApplyWindowInsetsListener(root) { _, windowInsets ->
      val insets = windowInsets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val density = resources.displayMetrics.density.takeIf { it > 0f } ?: 1f
      insetTop = insets.top / density
      insetBottom = insets.bottom / density
      applySafeAreaInsets()
      windowInsets
    }
    ViewCompat.requestApplyInsets(root)
  }

  /**
   * 把安全区尺寸（已换算为 CSS 像素）注入页面：
   * - 立即执行一次，修正当前页面
   * - 同时注册 document-start 脚本，保证页面重新加载后依然生效
   *
   * 前端使用 max(env(safe-area-inset-*), var(--win-inset-*)) 取值，
   * 用于兜底 WebView 140 之前 env(safe-area-inset-*) 恒为 0 的 Chromium 缺陷。
   */
  private fun applySafeAreaInsets() {
    val view = webView ?: return
    val js = safeAreaScript()
    if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
      insetsScript?.remove()
      insetsScript = WebViewCompat.addDocumentStartJavaScript(view, js, setOf("*"))
    }
    view.post { view.evaluateJavascript(js, null) }
  }

  private fun safeAreaScript(): String =
    "(function(){var s=document.documentElement.style;" +
      "s.setProperty('--win-inset-top','${insetTop}px');" +
      "s.setProperty('--win-inset-bottom','${insetBottom}px');})();"
}

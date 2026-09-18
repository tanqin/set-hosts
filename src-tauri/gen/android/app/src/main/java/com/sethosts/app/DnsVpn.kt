package com.sethosts.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.widget.Toast

/**
 * Rust(JNI) 与 [DnsVpnService] 之间的桥接。
 *
 * Rust 侧通过 [nativeInit] 注入 JavaVM 与 Activity 引用，之后直接在该 Activity 上调用
 * `startDnsVpn(Int)` / `stopDnsVpn()` / `isDnsVpnRunning()`（见 MainActivity）。
 */
object DnsVpn {
  /** VPN 授权请求码，配合 MainActivity.onActivityResult 使用 */
  private const val REQUEST_VPN = 0x5E70
  private const val DEFAULT_DNS_PORT = 5353

  private const val PREFS = "dns_vpn"
  /** 用户是否明确拒绝过 VPN 授权（拒绝后不再自动弹窗，避免每次启动打扰） */
  private const val KEY_CONSENT_DECLINED = "consent_declined"

  init {
    System.loadLibrary("tauri_app_lib")
  }

  @Volatile
  private var pendingPort = DEFAULT_DNS_PORT

  /** 由 MainActivity 在 Activity 创建时调用，把 JavaVM 与 Activity 引用交给 Rust */
  @JvmStatic
  external fun nativeInit(activity: Activity)

  /** 把系统 VPN 授权结果回传 Rust，Rust 再广播给前端（拒绝时要回滚配置开关） */
  @JvmStatic
  external fun nativeOnConsentResult(granted: Boolean)

  /**
   * 请求启动 DNS 隧道（用户把配置开关从「全部关闭」切到「打开」时由 Rust 调用）。
   *
   * 这是**用户主动发起**的动作，所以未授权时总会弹出系统 VPN 授权弹窗——即使用户
   * 之前拒绝过，也要重新询问，避免开关打开了却没有任何效果。
   */
  @JvmStatic
  fun start(activity: Activity, port: Int) {
    pendingPort = port
    activity.runOnUiThread {
      val consent = try {
        VpnService.prepare(activity)
      } catch (e: Exception) {
        null
      }
      if (consent != null) {
        // 首次（或用户曾拒绝后再次主动开启）：由系统弹窗询问，结果经
        // MainActivity.onActivityResult → nativeOnConsentResult 回传
        activity.startActivityForResult(consent, REQUEST_VPN)
      } else {
        startService(activity, port)
        // 系统此前已授权：不会弹窗，也就不会有 onActivityResult 回调。
        // 主动回传一次「已授权」，否则前端会一直等授权结果，成功提示永远补不上。
        runCatching { nativeOnConsentResult(true) }
      }
    }
  }

  /**
   * 自动启动 DNS 隧道（应用启动 / 回到前台时由 Rust 调用）。
   *
   * 系统对 VPN 授权是按包名**永久记忆**的，因此已授权过时这里完全静默：
   * - 未授权且用户从未拒绝过 → 弹出一次系统授权弹窗（首次即完成全部配置）；
   * - 用户拒绝过 → 不再自动打扰，只有用户主动打开配置开关才会重新询问。
   */
  @JvmStatic
  fun autoStart(activity: Activity, port: Int) {
    pendingPort = port
    activity.runOnUiThread {
      val consent = try {
        VpnService.prepare(activity)
      } catch (e: Exception) {
        null
      }
      if (consent == null) {
        startService(activity, port)
        return@runOnUiThread
      }
      if (prefs(activity).getBoolean(KEY_CONSENT_DECLINED, false)) return@runOnUiThread
      activity.startActivityForResult(consent, REQUEST_VPN)
    }
  }

  /** 停止 DNS 隧道 */
  @JvmStatic
  fun stop(activity: Activity) {
    activity.runOnUiThread {
      runCatching { activity.stopService(Intent(activity, DnsVpnService::class.java)) }
      // 保险：直接让当前运行中的 Service 实例 teardown + stopSelf。
      // 某些系统对 stopService 的响应有延迟，只靠 stopService 会导致状态栏 VPN 图标残留。
      runCatching { DnsVpnService.stopInstance() }
    }
  }

  /** 隧道是否已建立 */
  @JvmStatic
  fun isRunning(): Boolean = DnsVpnService.isRunning()

  /** 读取隧道诊断报告（由 Rust 通过 JNI 调用，供前端「诊断日志」页展示） */
  @JvmStatic
  fun diagnostics(context: Context): String {
    DnsVpnDiagnostics.init(context)
    return runCatching { DnsVpnDiagnostics.dump() }
      .getOrElse { "读取原生诊断日志失败: ${it.message}" }
  }

  /** 清空隧道诊断日志 */
  @JvmStatic
  fun clearDiagnostics(context: Context) {
    DnsVpnDiagnostics.init(context)
    runCatching { DnsVpnDiagnostics.clear() }
  }

  /** 由 MainActivity.onActivityResult 转发 */
  fun onActivityResult(activity: Activity, requestCode: Int, resultCode: Int) {
    if (requestCode != REQUEST_VPN) return
    val granted = resultCode == Activity.RESULT_OK
    val prefs = prefs(activity).edit()
    if (granted) {
      // 授权成功：系统会按包名永久记住，之后启动 / 回到前台都静默接管
      prefs.putBoolean(KEY_CONSENT_DECLINED, false).apply()
      startService(activity, pendingPort)
    } else {
      // 记住这次拒绝：启动 / 回到前台不再自动弹窗；用户再次把配置开关从关切到开时
      // 仍会重新询问（走 start()）
      prefs.putBoolean(KEY_CONSENT_DECLINED, true).apply()
    }
    DnsVpnDiagnostics.consentDeclined = !granted
    DnsVpnDiagnostics.log(if (granted) "系统 VPN 授权已通过" else "系统 VPN 授权被拒绝")
    // 把结果同步给 Rust → 前端：授权被拒绝时前端要把刚打开的配置开关回滚
    runCatching { nativeOnConsentResult(granted) }
  }

  private fun prefs(activity: Activity) =
    activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

  private fun startService(activity: Activity, port: Int) {
    DnsVpnDiagnostics.init(activity)
    val intent = Intent(activity, DnsVpnService::class.java)
      .putExtra(DnsVpnService.EXTRA_PORT, port)
    try {
      activity.startService(intent)
    } catch (e: Exception) {
      Toast.makeText(activity, "启动 VPN 隧道失败: ${e.message}", Toast.LENGTH_LONG).show()
    }
  }
}

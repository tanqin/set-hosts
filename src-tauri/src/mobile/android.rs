//! Android 原生桥接：通过 JNI 调用 `gen/android` 原生工程中的 Kotlin 实现。
//!
//! 普通 Android 应用不能绑定 53 端口，也无法让系统解析器直接访问 `127.0.0.1:5353`，
//! 因此这里采用「隧道内应答」的方案（Kotlin 侧见 `DnsVpnService.kt`）：
//!
//! 1. VpnService 隧道只声明 `10.111.222.0/30` 一条路由，并把隧道内的 DNS 服务器设为
//!    `10.111.222.2`；
//! 2. 系统解析器发往 `10.111.222.2:53` 的 UDP 报文被路由进隧道，Kotlin 从 tun 读出后
//!    转发给本机 `127.0.0.1:<port>`（即 [`crate::dns_proxy`] 的本地 DNS 服务器），
//!    再把应答重新封装写回隧道；
//! 3. 其余流量不经过隧道，对系统网络没有影响。
//!
//! 通讯方式：Kotlin 的 `DnsVpn.nativeInit(activity)` 在 Activity 创建时把 JavaVM 与
//! Activity 引用交给 Rust（见 [`Java_com_sethosts_app_DnsVpn_nativeInit`]）；之后 Rust
//! 直接在该 Activity 对象上调用 `startDnsVpn(Int)` / `stopDnsVpn()` / `isDnsVpnRunning()`，
//! 从而避开在非 Java 线程上 `FindClass` 带来的类加载问题。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::JavaVM as RawJavaVM;
use jni::{JNIEnv, JavaVM};

/// 最近一次从 Kotlin 侧读到的隧道状态（JNI 查询失败时的兜底值）
static TUNNEL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// `jni 0.21` 的 [`JavaVM`] 未实现 `Clone`，但每次按需 `unsafe { JavaVM::from_raw(ptr) }`
/// 从裸指针重建即可（同一 JVM 上多份 `JavaVM` 句柄是合法的）。
/// 这里把 `*mut sys::JavaVM` 转成 `usize` 缓存，避免嵌套裸指针需要自定义 `Send` 实现。
static JVM_PTR: AtomicUsize = AtomicUsize::new(0);
static ACTIVITY: Mutex<Option<GlobalRef>> = Mutex::new(None);

/// Kotlin 侧 `DnsVpn.nativeInit(activity)` 在 Activity 创建时调用，注入 JavaVM 与 Activity 引用
#[no_mangle]
pub extern "system" fn Java_com_sethosts_app_DnsVpn_nativeInit(
    env: JNIEnv,
    _class: JClass,
    activity: JObject,
) {
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(err) => {
            log::warn!("缓存 JavaVM 失败: {err}");
            return;
        }
    };
    match env.new_global_ref(activity) {
        Ok(activity) => {
            JVM_PTR.store(vm.get_java_vm_pointer() as usize, Ordering::Release);
            if let Ok(mut slot) = ACTIVITY.lock() {
                *slot = Some(activity);
            }
            TUNNEL_ACTIVE.store(false, Ordering::Relaxed);
            log::info!("Android VPN 桥接已就绪");
        }
        Err(err) => log::warn!("缓存 Activity 引用失败: {err}"),
    }
}

/// 原生桥接（JavaVM + Activity）是否已就绪。
///
/// `DnsVpn.nativeInit` 由 MainActivity 在 `onCreate` 中调用，在此之前任何 JNI
/// 调用都会失败；因此自动建立隧道前需要先等它就绪。
pub fn is_ready() -> bool {
    if JVM_PTR.load(Ordering::Acquire) == 0 {
        return false;
    }
    ACTIVITY
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

/// 取出已缓存的 JavaVM 与 Activity 引用（按裸指针重建 `JavaVM`）
fn bridge() -> Result<(JavaVM, GlobalRef), String> {
    let raw = JVM_PTR.load(Ordering::Acquire) as *mut RawJavaVM;
    if raw.is_null() {
        return Err("原生桥接尚未初始化（DnsVpn.nativeInit 未调用）".to_string());
    }
    // SAFETY: 来自 `JNIEnv::get_java_vm()`，且在整个进程生命周期内有效
    let vm = unsafe { JavaVM::from_raw(raw) }
        .map_err(|err| format!("重建 JavaVM 句柄失败: {err}"))?;
    let activity = ACTIVITY
        .lock()
        .map_err(|_| "原生桥接状态已损坏".to_string())?
        .clone()
        .ok_or_else(|| "未缓存 Activity 引用".to_string())?;
    Ok((vm, activity))
}

/// 启动 VPN 隧道，把系统 DNS 指向本地 DNS 服务器端口
pub fn start_vpn(port: u16) -> Result<(), String> {
    let (vm, activity) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
    env.call_method(
        activity,
        "startDnsVpn",
        "(I)V",
        &[JValue::Int(i32::from(port))],
    )
    .map_err(|err| format!("调用原生 VPN 隧道启动失败: {err}"))?;
    Ok(())
}

/// 自动启动 VPN 隧道（应用启动 / 回到前台时调用）。
///
/// 与 [`start_vpn`] 的区别在于不会打扰用户：已授权时静默接管；用户从未授权过则
/// 弹出一次系统授权弹窗；用户曾拒绝过就不再自动弹窗——只有用户主动把配置开关
/// 从关切到开（走 [`start_vpn`]）才会重新申请授权。
pub fn auto_start_vpn(port: u16) -> Result<(), String> {
    let (vm, activity) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
    env.call_method(
        activity,
        "autoStartDnsVpn",
        "(I)V",
        &[JValue::Int(i32::from(port))],
    )
    .map_err(|err| format!("调用原生 VPN 隧道自动启动失败: {err}"))?;
    Ok(())
}

/// Kotlin 侧 `DnsVpn.onActivityResult` 拿到系统 VPN 授权结果后回调本函数。
///
/// 调用发生在 Java→native 的上行调用里（线程已挂载在 JVM 上），因此无需再 attach；
/// 这里只把结果转成 Tauri 事件转发给前端：授权被拒绝时前端要把刚打开的配置开关回滚。
#[no_mangle]
pub extern "system" fn Java_com_sethosts_app_DnsVpn_nativeOnConsentResult(
    _env: JNIEnv,
    _class: JClass,
    granted: jni::sys::jboolean,
) {
    crate::mobile::notify_consent_result(granted != 0);
}

/// 停止 VPN 隧道
pub fn stop_vpn() -> Result<(), String> {
    let (vm, activity) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
    env.call_method(activity, "stopDnsVpn", "()V", &[])
        .map_err(|err| format!("调用原生 VPN 隧道停止失败: {err}"))?;
    TUNNEL_ACTIVE.store(false, Ordering::Relaxed);
    Ok(())
}

/// 读取原生隧道诊断报告（Kotlin 侧 `DnsVpnDiagnostics` 的内存缓冲 + 落盘日志）
pub fn diagnostics_dump() -> Result<String, String> {
    use jni::objects::JString;

    let (vm, activity) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
    let value = env
        .call_method(activity, "vpnDiagnostics", "()Ljava/lang/String;", &[])
        .map_err(|err| format!("读取原生诊断日志失败: {err}"))?
        .l()
        .map_err(|err| format!("解析原生诊断日志返回类型失败: {err}"))?;
    if value.is_null() {
        return Ok(String::new());
    }
    let text: String = env
        .get_string(&JString::from(value))
        .map_err(|err| format!("转换原生诊断日志失败: {err}"))?
        .into();
    Ok(text)
}

/// 清空原生隧道诊断日志
pub fn diagnostics_clear() -> Result<(), String> {
    let (vm, activity) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
    env.call_method(activity, "clearVpnDiagnostics", "()V", &[])
        .map_err(|err| format!("清空原生诊断日志失败: {err}"))?;
    Ok(())
}

/// VPN 隧道是否已接管系统 DNS
pub fn is_vpn_active() -> bool {
    let result = bridge().and_then(|(vm, activity)| {
        let mut env = vm
            .attach_current_thread()
            .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
        env.call_method(activity, "isDnsVpnRunning", "()Z", &[])
            .map_err(|err| format!("查询原生 VPN 隧道状态失败: {err}"))?
            .z()
            .map_err(|err| format!("解析原生 VPN 隧道状态失败: {err}"))
    });
    match result {
        Ok(active) => {
            TUNNEL_ACTIVE.store(active, Ordering::Relaxed);
            active
        }
        Err(err) => {
            log::debug!("{err}");
            TUNNEL_ACTIVE.load(Ordering::Relaxed)
        }
    }
}

/// 退出应用（前端「再按一次退出」确认后调用）：
/// 结束 Activity 并终止进程（Kotlin 侧 `MainActivity.exitApp()`，
/// 会先停掉 VPN 隧道再杀进程，避免残留进程导致下次启动白屏）
pub fn exit_app() -> Result<(), String> {
    let (vm, activity) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|err| format!("挂载当前线程到 JVM 失败: {err}"))?;
    env.call_method(activity, "exitApp", "()V", &[])
        .map_err(|err| format!("调用原生退出失败: {err}"))?;
    Ok(())
}

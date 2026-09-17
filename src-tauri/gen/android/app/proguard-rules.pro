# ============================================================================
# Rust ↔ Kotlin JNI 桥接符号必须保留名称（Rust 端按字符串查找）
# 否则 R8 重命名后会出现 UnsatisfiedLinkError / NoSuchMethodError 导致启动崩溃
# ============================================================================

# DnsVpn 是 Kotlin object，nativeInit 是 external 函数，对应 Rust 端
# Java_com_sethosts_app_DnsVpn_nativeInit（pub extern "system" + #[no_mangle]）。
# 整个类不能改名也不能删字段（含 INSTANCE 与 <clinit> 里的 System.loadLibrary）。
-keep class com.sethosts.app.DnsVpn {
    public static native void nativeInit(android.app.Activity);
}
-keepclassmembers class com.sethosts.app.DnsVpn {
    static <fields>;
    static <clinit>();
}

# MainActivity 的 VPN 控制方法被 Rust 通过 env.call_method 按方法名查找。
# 静态分析看不出 JNI 调用，R8 会把它们当无用代码删掉。
#
# 这里整类保留，而不是逐个列方法名：名单式规则只要新增一个 JNI 方法忘了登记，
# 就会在 release 包里变成 NoSuchMethodError（debug 包看不出来），排查成本极高。
-keep class com.sethosts.app.MainActivity { *; }

# DnsVpnService 整个保留，避免 R8 移除影响后续 JNI 回调。
-keep class com.sethosts.app.DnsVpnService { *; }
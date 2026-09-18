//! Set Hosts - 跨平台 Hosts 配置工具
//!
//! 桌面端（Linux/macOS/Windows）：直接读写系统 hosts 文件，带权限提升。
//! 移动端（Android/iOS）：通过本地 DNS 代理 + VPN 隧道让映射生效，无需 root/越狱。

pub mod commands;
pub mod dns_flush;
pub mod dns_proxy;
pub mod hosts_path;
pub mod import_export;
pub mod mobile;
pub mod models;
pub mod parser;
pub mod privilege;
pub mod process;
pub mod store;

use commands::AppState;
use tauri::Manager;

/// 显示并聚焦主窗口（托盘 / 二次启动复用）
/// 仅桌面端使用（移动端没有托盘与多实例，unminimize 等方法也不可用）
#[cfg(desktop)]
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 创建系统托盘（桌面端）：菜单含"显示主窗口 / 退出"，左键点击显示窗口
#[cfg(desktop)]
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    // 托盘菜单文案跟随已保存的语言设置（与前端 LOCALES 保持一致，新增语言时这里也要追加）
    let lang = store::load_settings_public(app.handle()).language;
    let (show_label, quit_label) = match lang.as_str() {
        "en" => ("Show Main Window", "Quit"),
        "ja" => ("メインウィンドウを表示", "終了"),
        "ko" => ("메인 창 표시", "종료"),
        "de" => ("Hauptfenster anzeigen", "Beenden"),
        "fr" => ("Afficher la fenêtre principale", "Quitter"),
        "es" => ("Mostrar ventana principal", "Salir"),
        "zh-TW" => ("顯示主視窗", "退出"),
        "pt" => ("Mostrar janela principal", "Sair"),
        // 默认：简体中文（也覆盖未识别的旧值）
        _ => ("显示主窗口", "退出"),
    };

    let show = tauri::menu::MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
    let quit = tauri::menu::MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    let menu = tauri::menu::Menu::with_items(app, &[&show, &quit])?;

    tauri::tray::TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("缺少应用图标").clone())
        .tooltip("Set Hosts")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}


/// Windows: 检测管理员权限，不足则自动提权重启
/// dev 模式下用 -Wait 等待提权进程退出，保持原进程存活避免 Tauri CLI 关闭 Vite
#[cfg(target_os = "windows")]
fn ensure_admin() {
    // 用 net session 检测当前是否有管理员权限
    let is_admin = crate::process::hidden(std::process::Command::new("cmd"))
        .args(["/c", "net session"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if is_admin {
        return;
    }

    // 通过 PowerShell Start-Process -Verb RunAs 触发 UAC 提权重启
    // -Wait: 等待提权进程退出后再返回，保持原进程存活
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy().replace('\'', "''");
        let ps_cmd = format!("Start-Process -FilePath '{}' -Verb RunAs -Wait", exe_str);
        let _ = crate::process::hidden(std::process::Command::new("powershell"))
            .args(["-NoProfile", "-Command", &ps_cmd])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        // 提权进程退出或 UAC 被取消，退出当前进程
        std::process::exit(0);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Windows: 以管理员权限启动（dev + release）
    #[cfg(target_os = "windows")]
    ensure_admin();

    // Android: 把 log crate 接到 logcat，真机排查用 `adb logcat -s SetHosts`
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            // logcat 的 tag 不能有空格（且长度上限 23），用去掉空格的 SetHosts
            .with_tag("SetHosts"),
    );

    let builder = tauri::Builder::default();

    // 桌面端插件：单实例必须最先注册，二次启动时唤起已有窗口
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // 加载持久化配置，初始化全局状态
            let config = match store::load_config(app.handle()) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("加载配置失败，使用默认配置: {}", e);
                    models::Config::default()
                }
            };
            app.manage(AppState::new(config));

            // 启动时隐藏窗口（窗口默认 visible: false，未开启则手动显示）
            // 移动端不适用"启动隐藏"（无托盘可唤回，会导致白屏），始终显示
            let settings = store::load_settings_public(app.handle());
            let show_on_startup = if cfg!(desktop) {
                !settings.hide_on_startup
            } else {
                true
            };
            if show_on_startup {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            // 系统托盘（桌面端）
            #[cfg(desktop)]
            setup_tray(app)?;

            // 后台自动刷新已启用的远程 hosts（受"启动时自动刷新"设置控制）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                commands::auto_refresh_remote_profiles(handle).await;
            });

            // 定时自动刷新：按各远程 profile 自身的「自动刷新」间隔轮询
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                commands::spawn_remote_auto_refresh_loop(handle).await;
            });

            // DNS 代理：按设置自动启动（移动端映射生效的前提）
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                commands::auto_start_dns_proxy(handle).await;
            });

            // 移动端：把 AppHandle 交给原生桥接层，供 Kotlin 侧回传系统 VPN
            // 授权结果时向前端广播 vpn-consent 事件（授权被拒绝需回滚配置开关）
            #[cfg(target_os = "android")]
            mobile::init_app_handle(app.handle().clone());

            Ok(())
        })
        // 桌面端：点关闭按钮隐藏到托盘，通过托盘菜单退出
        .on_window_event(|_window, _event| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = _event {
                let _ = _window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Profile 管理
            commands::get_profiles,
            commands::get_active_profile,
            commands::set_active_profile,
            commands::create_profile,
            commands::delete_profile,
            commands::rename_profile,
            commands::create_remote_profile,
            commands::refresh_remote_profile,
            commands::update_remote_profile,
            commands::get_profile_content,
            commands::save_profile_content,
            commands::toggle_profile,
            commands::apply_profile,
            // 备份还原
            commands::backup_hosts,
            commands::list_backups,
            commands::restore_backup,
            // 读取当前 hosts
            commands::get_current_hosts_content,
            // 导入导出
            commands::export_config,
            commands::import_config,
            commands::export_config_to_file,
            commands::import_config_from_file,
            // DNS 代理
            commands::start_dns_proxy,
            commands::stop_dns_proxy,
            commands::get_proxy_status,
            commands::ensure_tunnel,
            // 平台信息
            commands::get_platform_info,
            // 诊断日志
            commands::get_diagnostics,
            commands::clear_diagnostics,
            // 打开文件夹 & 数据目录
            commands::open_hosts_folder,
            commands::get_data_dir,
            commands::change_data_dir,
            // 应用设置
            commands::get_app_settings,
            commands::save_app_settings,
            // 开机自启
            commands::get_autostart_status,
            commands::set_autostart,
            // 退出应用（移动端「再按一次退出」）
            commands::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Set Hosts application");
}

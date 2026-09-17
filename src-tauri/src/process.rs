//! 子进程辅助

use std::process::Command;

/// 以「无控制台窗口」的方式启动子进程，返回同一个 Command 便于链式调用
///
/// 本程序是 GUI 子系统程序（见 main.rs 的 `windows_subsystem = "windows"`），
/// 自身没有控制台。此时直接 spawn 控制台程序（`cmd` / `powershell` / `ipconfig` …），
/// Windows 会为子进程新分配一个控制台窗口，表现为屏幕上一闪而过的黑色终端框。
/// `CREATE_NO_WINDOW` 让子进程不分配控制台，stdout/stderr 仍可通过管道正常读取。
///
/// 其它平台没有这个概念，本函数是空操作。
pub fn hidden(mut command: Command) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        /// 不为子进程分配控制台窗口
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

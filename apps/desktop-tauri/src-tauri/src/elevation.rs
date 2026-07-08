#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        unsafe { IsUserAnAdmin().as_bool() }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn restart_as_admin() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe()?;
        let exe_wide: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("runas"),
                windows::core::PCWSTR(exe_wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            anyhow::bail!("Windows refused administrator restart");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("administrator restart is only supported on Windows")
    }
}

use crate::app_state::AppMode;
use anyhow::{Context, Result};
use std::process::Command;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Devices Router";

pub fn set_start_on_login(enabled: bool, mode: AppMode) -> Result<()> {
    if enabled {
        let exe = std::env::current_exe().context("获取当前程序路径失败")?;
        let mode_arg = match mode {
            AppMode::Host => "--host",
            AppMode::Remote => "--remote",
            AppMode::Idle => "--host",
        };
        let value = format!("\"{}\" {mode_arg}", exe.display());
        run_reg(&[
            "add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", &value, "/f",
        ])?;
    } else {
        let _ = run_reg(&["delete", RUN_KEY, "/v", VALUE_NAME, "/f"]);
    }
    Ok(())
}

fn run_reg(args: &[&str]) -> Result<()> {
    let output = Command::new("reg.exe")
        .args(args)
        .output()
        .context("执行注册表命令失败")?;
    if !output.status.success() {
        anyhow::bail!(
            "注册表命令失败：{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

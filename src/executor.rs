use anyhow::Result;
use colored::*;
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::config;
use crate::danger::DangerLevel;
use crate::i18n;
use crate::ui;

#[derive(Debug, Serialize, Deserialize)]
pub struct LastExec {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timestamp: u64,
}

pub enum UserChoice {
    Execute,
    Cancel,
    Edit(String),
}

pub fn prompt_user(
    command: &str,
    danger: DangerLevel,
    auto_confirm: bool,
    tr: &i18n::T,
    explanation: Option<&str>,
) -> Result<UserChoice> {
    // Show danger warnings
    match danger {
        DangerLevel::Dangerous => ui::print_danger(tr),
        DangerLevel::Warning => ui::print_warning(tr),
        DangerLevel::Safe => {}
    }

    ui::print_command(command);
    println!();

    // Show explanation if provided
    if let Some(expl) = explanation {
        if !expl.is_empty() {
            ui::print_explanation(tr, expl);
        }
    }

    // Auto-confirm safe commands if configured
    if auto_confirm && danger == DangerLevel::Safe {
        return Ok(UserChoice::Execute);
    }

    // Dangerous commands: always require explicit confirmation, cannot be skipped
    if danger == DangerLevel::Dangerous {
        let items = vec![tr.yes_execute, tr.no_cancel, tr.edit_command];
        let selection = Select::new()
            .with_prompt(tr.confirm_dangerous.red().bold().to_string())
            .items(&items)
            .default(1) // Default to cancel
            .interact()?;

        return match selection {
            0 => Ok(UserChoice::Execute),
            2 => {
                let edited: String = Input::new()
                    .with_prompt(tr.edit_prompt)
                    .with_initial_text(command)
                    .interact_text()?;
                Ok(UserChoice::Edit(edited))
            }
            _ => Ok(UserChoice::Cancel),
        };
    }

    // Normal prompt for safe/warning
    let items = vec![tr.execute, tr.cancel, tr.edit];
    let selection = Select::new().items(&items).default(0).interact()?;

    match selection {
        0 => Ok(UserChoice::Execute),
        2 => {
            let edited: String = Input::new()
                .with_prompt(tr.edit_prompt)
                .with_initial_text(command)
                .interact_text()?;
            Ok(UserChoice::Edit(edited))
        }
        _ => Ok(UserChoice::Cancel),
    }
}

/// Convenience wrapper that auto-detects the shell. Used by integration tests.
#[allow(dead_code)]
pub fn execute_command(command: &str, tr: &i18n::T) -> Result<(i32, String, String)> {
    execute_command_with_shell(command, "", tr)
}

/// Execute a command using the specified shell.
/// If `shell` is provided, it determines how the command is executed on Windows.
/// If empty, falls back to auto-detection.
pub fn execute_command_with_shell(
    command: &str,
    shell: &str,
    tr: &i18n::T,
) -> Result<(i32, String, String)> {
    let shell_cmd = if cfg!(target_os = "windows") {
        match shell {
            "PowerShell" => run_powershell(command),
            "cmd" => run_cmd(command),
            "bash" | "zsh" | "fish" => run_unix_like(command),
            _ => {
                // Fallback: auto-detect (for callers that don't have context)
                if is_powershell_parent() {
                    run_powershell(command)
                } else {
                    run_cmd(command)
                }
            }
        }
    } else {
        Command::new("sh").args(["-c", command]).output()
    };

    let output = shell_cmd?;
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = decode_output(&output.stdout);
    let stderr = decode_output(&output.stderr);

    if !stdout.is_empty() {
        print!("{}", stdout);
    }
    if !stderr.is_empty() {
        eprint!("{}", stderr.dimmed());
    }

    save_last_exec(command, exit_code, &stdout, &stderr)?;

    if exit_code != 0 {
        println!(
            "\n{} {}: {}",
            "✗".red(),
            tr.exit_code,
            exit_code.to_string().red()
        );
    }

    Ok((exit_code, stdout, stderr))
}

fn save_last_exec(command: &str, exit_code: i32, stdout: &str, stderr: &str) -> Result<()> {
    let dir = config::piz_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("last_exec.json");

    // Keep only first 500 chars of output to avoid huge files
    let stdout_preview: String = stdout.chars().take(500).collect();
    let stderr_preview: String = stderr.chars().take(500).collect();

    let last = LastExec {
        command: command.to_string(),
        exit_code,
        stdout: stdout_preview,
        stderr: stderr_preview,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    let json = serde_json::to_string_pretty(&last)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Run command through PowerShell without modifying encoding.
/// Output bytes are decoded by `decode_output()` (UTF-8 first, GBK fallback).
fn run_powershell(command: &str) -> std::io::Result<std::process::Output> {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", command])
        .output()
}

/// Run command through cmd.exe without modifying codepage.
/// Output bytes are decoded by `decode_output()` (UTF-8 first, GBK fallback).
fn run_cmd(command: &str) -> std::io::Result<std::process::Output> {
    Command::new("cmd").args(["/C", command]).output()
}

/// Run command through sh (for Git Bash / MSYS2 on Windows).
fn run_unix_like(command: &str) -> std::io::Result<std::process::Output> {
    Command::new("sh").args(["-c", command]).output()
}

/// Check if the parent process is PowerShell (fallback detection).
fn is_powershell_parent() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Use the same parent-process detection as context.rs
        crate::context::detect_windows_parent_shell()
            .map(|s| s == "PowerShell")
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Decode command output bytes to String.
/// On Windows, if UTF-8 decode fails, try GBK (CP936) for Chinese Windows.
fn decode_output(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            // Fallback: try GBK decoding for Chinese Windows
            #[cfg(target_os = "windows")]
            {
                decode_gbk(bytes)
            }
            #[cfg(not(target_os = "windows"))]
            {
                String::from_utf8_lossy(bytes).to_string()
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn decode_gbk(bytes: &[u8]) -> String {
    // Simple GBK → UTF-8: use Windows API MultiByteToWideChar
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let codepage = 936; // GBK
        let len = windows_sys::Win32::Globalization::MultiByteToWideChar(
            codepage,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        );
        if len <= 0 {
            return String::from_utf8_lossy(bytes).to_string();
        }
        let mut wide: Vec<u16> = vec![0; len as usize];
        let written = windows_sys::Win32::Globalization::MultiByteToWideChar(
            codepage,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            len,
        );
        if written <= 0 {
            return String::from_utf8_lossy(bytes).to_string();
        }
        wide.truncate(written as usize);
        OsString::from_wide(&wide).to_string_lossy().to_string()
    }
}

pub fn load_last_exec() -> Result<LastExec> {
    let path = config::piz_dir()?.join("last_exec.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|_| anyhow::anyhow!("No previous command execution found."))?;
    let last: LastExec = serde_json::from_str(&content)?;
    Ok(last)
}

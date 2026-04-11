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
    Regenerate,
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

    // Auto-confirm only for a conservative local read-only whitelist.
    if should_auto_confirm(command, danger, auto_confirm) {
        return Ok(UserChoice::Execute);
    }

    // Dangerous commands: always require explicit confirmation, cannot be skipped
    if danger == DangerLevel::Dangerous {
        let items = vec![tr.yes_execute, tr.no_cancel, tr.edit_command, tr.regenerate];
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
            3 => Ok(UserChoice::Regenerate),
            _ => Ok(UserChoice::Cancel),
        };
    }

    // Normal prompt for safe/warning
    let items = vec![tr.execute, tr.cancel, tr.edit, tr.regenerate_short];
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
        3 => Ok(UserChoice::Regenerate),
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
        run_windows_command(command, shell)
    } else {
        run_unix_command(command, shell)
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

fn should_auto_confirm(command: &str, danger: DangerLevel, auto_confirm: bool) -> bool {
    auto_confirm && danger == DangerLevel::Safe && is_auto_confirm_whitelisted(command)
}

fn is_auto_confirm_whitelisted(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }

    if contains_risky_shell_constructs(trimmed) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let first = lower.split_whitespace().next().unwrap_or("");

    match first {
        "ls" | "dir" | "pwd" | "whoami" | "df" | "du" | "ps" | "cat" | "type" => true,
        "git" => matches_git_read_only(trimmed),
        "docker" => matches_docker_read_only(trimmed),
        "echo" => matches_echo_read_only(trimmed),
        _ => false,
    }
}

fn contains_risky_shell_constructs(command: &str) -> bool {
    ['|', '>', '<', ';', '&']
        .iter()
        .any(|ch| command.contains(*ch))
}

fn matches_git_read_only(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    matches!(
        lower.split_whitespace().nth(1),
        Some("status" | "log" | "diff" | "show" | "branch")
    )
}

fn matches_docker_read_only(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    matches!(
        lower.split_whitespace().nth(1),
        Some("ps" | "images" | "inspect" | "logs")
    )
}

fn matches_echo_read_only(command: &str) -> bool {
    !command.contains('$') && !command.contains('%')
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

fn run_bash(command: &str) -> std::io::Result<std::process::Output> {
    Command::new("bash").args(["-c", command]).output()
}

fn run_zsh(command: &str) -> std::io::Result<std::process::Output> {
    Command::new("zsh").args(["-c", command]).output()
}

fn run_fish(command: &str) -> std::io::Result<std::process::Output> {
    Command::new("fish").args(["-c", command]).output()
}

fn run_unix_command(command: &str, shell: &str) -> std::io::Result<std::process::Output> {
    match resolve_unix_shell(shell).as_str() {
        "bash" => run_bash(command),
        "zsh" => run_zsh(command),
        "fish" => run_fish(command),
        _ => run_unix_like(command),
    }
}

fn run_windows_command(command: &str, shell: &str) -> std::io::Result<std::process::Output> {
    match resolve_windows_shell(shell).as_str() {
        "PowerShell" => run_powershell(command),
        "cmd" => run_cmd(command),
        "bash" => run_bash(command),
        "zsh" => run_zsh(command),
        "fish" => run_fish(command),
        _ => run_cmd(command),
    }
}

fn resolve_unix_shell(shell: &str) -> String {
    let normalized = normalize_shell_name(shell);
    if normalized.is_empty() {
        return first_available_shell(&["bash", "zsh", "fish", "sh"]);
    }

    match normalized.as_str() {
        "bash" if is_unix_shell_available("bash") => "bash".to_string(),
        "zsh" if is_unix_shell_available("zsh") => "zsh".to_string(),
        "fish" if is_unix_shell_available("fish") => "fish".to_string(),
        "sh" => "sh".to_string(),
        _ => first_available_shell(&["bash", "zsh", "fish", "sh"]),
    }
}

fn resolve_windows_shell(shell: &str) -> String {
    let normalized = normalize_shell_name(shell);
    match normalized.as_str() {
        "powershell" if is_windows_shell_available("powershell") => "PowerShell".to_string(),
        "cmd" if is_windows_shell_available("cmd") => "cmd".to_string(),
        "bash" if is_unix_shell_available("bash") => "bash".to_string(),
        "zsh" if is_unix_shell_available("zsh") => "zsh".to_string(),
        "fish" if is_unix_shell_available("fish") => "fish".to_string(),
        _ => {
            if is_powershell_parent() && is_windows_shell_available("powershell") {
                "PowerShell".to_string()
            } else {
                "cmd".to_string()
            }
        }
    }
}

fn first_available_shell(candidates: &[&str]) -> String {
    for candidate in candidates {
        if *candidate == "sh" || is_unix_shell_available(candidate) {
            return (*candidate).to_string();
        }
    }
    "sh".to_string()
}

fn normalize_shell_name(shell: &str) -> String {
    match shell.trim().to_ascii_lowercase().as_str() {
        "powershell" | "pwsh" => "powershell".to_string(),
        "cmd" => "cmd".to_string(),
        "bash" => "bash".to_string(),
        "zsh" => "zsh".to_string(),
        "fish" => "fish".to_string(),
        "sh" => "sh".to_string(),
        _ => String::new(),
    }
}

fn is_unix_shell_available(shell: &str) -> bool {
    match Command::new(shell).arg("--version").output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn is_windows_shell_available(shell: &str) -> bool {
    match shell {
        "powershell" => Command::new("powershell")
            .args(["-NoProfile", "-Command", "$PSVersionTable.PSVersion.Major"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false),
        "cmd" => Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false),
        _ => false,
    }
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

    // SAFETY: MultiByteToWideChar is a standard Windows API for codepage conversion.
    // We call it twice: first with null output buffer to get required length, then with
    // a properly sized buffer. Return values are checked — on failure we fall back to
    // lossy UTF-8 conversion. No aliasing or lifetime issues: `bytes` is borrowed
    // immutably and `wide` is a local owned buffer.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── decode_output ──

    #[test]
    fn decode_valid_utf8() {
        assert_eq!(decode_output(b"hello world"), "hello world");
    }

    #[test]
    fn decode_empty_bytes() {
        assert_eq!(decode_output(&[]), "");
    }

    #[test]
    fn decode_utf8_chinese() {
        let input = "你好世界".as_bytes();
        assert_eq!(decode_output(input), "你好世界");
    }

    #[test]
    fn decode_invalid_utf8_does_not_panic() {
        let input = vec![0xFF, 0xFE, 0x41];
        let result = decode_output(&input);
        assert!(!result.is_empty());
    }

    #[test]
    fn decode_utf8_with_newlines() {
        assert_eq!(decode_output(b"line1\nline2\n"), "line1\nline2\n");
    }

    // ── LastExec serialization ──

    #[test]
    fn last_exec_serialization_roundtrip() {
        let last = LastExec {
            command: "echo test".into(),
            exit_code: 0,
            stdout: "test\n".into(),
            stderr: "".into(),
            timestamp: 1234567890,
        };
        let json = serde_json::to_string_pretty(&last).unwrap();
        let loaded: LastExec = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.command, "echo test");
        assert_eq!(loaded.exit_code, 0);
        assert_eq!(loaded.stdout, "test\n");
        assert_eq!(loaded.stderr, "");
        assert_eq!(loaded.timestamp, 1234567890);
    }

    #[test]
    fn last_exec_has_all_json_fields() {
        let last = LastExec {
            command: "ls".into(),
            exit_code: 1,
            stdout: "out".into(),
            stderr: "err".into(),
            timestamp: 999,
        };
        let json = serde_json::to_string(&last).unwrap();
        assert!(json.contains("\"command\""));
        assert!(json.contains("\"exit_code\""));
        assert!(json.contains("\"stdout\""));
        assert!(json.contains("\"stderr\""));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn stdout_preview_truncates_at_500_chars() {
        let long_output: String = "x".repeat(1000);
        let preview: String = long_output.chars().take(500).collect();
        assert_eq!(preview.len(), 500);
    }

    #[test]
    fn stdout_preview_short_unchanged() {
        let short = "hello";
        let preview: String = short.chars().take(500).collect();
        assert_eq!(preview, "hello");
    }

    // ── prompt_user auto-confirm shortcut ──

    #[test]
    fn auto_confirm_safe_returns_execute() {
        let tr = crate::i18n::t(crate::i18n::Lang::En);
        let result = prompt_user("echo hi", DangerLevel::Safe, true, tr, None).unwrap();
        assert!(matches!(result, UserChoice::Execute));
    }

    #[test]
    fn auto_confirm_safe_with_explanation() {
        let tr = crate::i18n::t(crate::i18n::Lang::Zh);
        let result = prompt_user(
            "echo hi",
            DangerLevel::Safe,
            true,
            tr,
            Some("echo: 输出文本"),
        )
        .unwrap();
        assert!(matches!(result, UserChoice::Execute));
    }

    #[test]
    fn should_auto_confirm_whitelisted_read_only_commands() {
        assert!(should_auto_confirm("ls -la", DangerLevel::Safe, true));
        assert!(should_auto_confirm("pwd", DangerLevel::Safe, true));
        assert!(should_auto_confirm("git status", DangerLevel::Safe, true));
        assert!(should_auto_confirm("docker ps", DangerLevel::Safe, true));
    }

    #[test]
    fn should_not_auto_confirm_non_whitelisted_safe_commands() {
        assert!(!should_auto_confirm(
            "git checkout -b feature/test",
            DangerLevel::Safe,
            true
        ));
        assert!(!should_auto_confirm(
            "curl https://example.com",
            DangerLevel::Safe,
            true
        ));
        assert!(!should_auto_confirm(
            "docker exec -it app sh",
            DangerLevel::Safe,
            true
        ));
    }

    #[test]
    fn should_not_auto_confirm_risky_shell_constructs() {
        assert!(!should_auto_confirm(
            "echo hi > out.txt",
            DangerLevel::Safe,
            true
        ));
        assert!(!should_auto_confirm(
            "git status | cat",
            DangerLevel::Safe,
            true
        ));
        assert!(!should_auto_confirm("echo $HOME", DangerLevel::Safe, true));
    }

    #[test]
    fn should_not_auto_confirm_when_disabled_or_non_safe() {
        assert!(!should_auto_confirm("ls -la", DangerLevel::Safe, false));
        assert!(!should_auto_confirm("ls -la", DangerLevel::Warning, true));
    }

    // ── execute_command ──

    #[test]
    fn execute_echo_captures_stdout() {
        let tr = crate::i18n::t(crate::i18n::Lang::En);
        let (code, stdout, _) = execute_command("echo piz_unit_test", tr).unwrap();
        assert_eq!(code, 0);
        assert!(stdout.contains("piz_unit_test"));
    }

    #[test]
    fn execute_failing_returns_nonzero() {
        let tr = crate::i18n::t(crate::i18n::Lang::En);
        let cmd = if cfg!(target_os = "windows") {
            "cmd /C exit 42"
        } else {
            "exit 42"
        };
        let (code, _, _) = execute_command(cmd, tr).unwrap();
        assert_ne!(code, 0);
    }

    #[test]
    fn execute_captures_stderr() {
        let tr = crate::i18n::t(crate::i18n::Lang::En);
        let cmd = if cfg!(target_os = "windows") {
            "echo error 1>&2"
        } else {
            "echo error >&2"
        };
        let (_, _, stderr) = execute_command(cmd, tr).unwrap();
        assert!(stderr.contains("error"));
    }

    #[test]
    fn normalize_shell_name_variants() {
        assert_eq!(normalize_shell_name("PowerShell"), "powershell");
        assert_eq!(normalize_shell_name("pwsh"), "powershell");
        assert_eq!(normalize_shell_name("bash"), "bash");
        assert_eq!(normalize_shell_name("unknown"), "");
    }

    #[test]
    fn resolve_unix_shell_prefers_requested_shell_when_available() {
        if is_unix_shell_available("bash") {
            assert_eq!(resolve_unix_shell("bash"), "bash");
        }
    }

    #[test]
    fn resolve_unix_shell_falls_back_to_available_shell() {
        let resolved = resolve_unix_shell("missing-shell-name");
        assert!(matches!(resolved.as_str(), "bash" | "zsh" | "fish" | "sh"));
    }

    #[test]
    fn resolve_windows_shell_normalizes_powershell() {
        if cfg!(target_os = "windows") {
            let resolved = resolve_windows_shell("pwsh");
            assert!(matches!(resolved.as_str(), "PowerShell" | "cmd"));
        }
    }

    #[test]
    fn execute_with_requested_shell_runs_successfully() {
        let tr = crate::i18n::t(crate::i18n::Lang::En);
        if cfg!(target_os = "windows") {
            let (code, stdout, _) =
                execute_command_with_shell("echo executor_shell_test", "cmd", tr).unwrap();
            assert_eq!(code, 0);
            assert!(stdout.contains("executor_shell_test"));
        } else {
            let (code, stdout, _) =
                execute_command_with_shell("echo executor_shell_test", "bash", tr).unwrap();
            assert_eq!(code, 0);
            assert!(stdout.contains("executor_shell_test"));
        }
    }
}

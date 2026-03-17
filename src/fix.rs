use anyhow::Result;

use crate::context::SystemContext;
use crate::danger::{self, DangerLevel};
use crate::executor::{self, UserChoice};
use crate::history;
use crate::i18n;
use crate::llm::prompt::build_fix_prompt;
use crate::llm::LlmBackend;
use crate::ui;

/// Maximum auto-fix retry attempts
pub const MAX_AUTO_FIX_RETRIES: usize = 3;

pub async fn fix_last_command(
    backend: &dyn LlmBackend,
    ctx: &SystemContext,
    tr: &i18n::T,
    lang: &str,
) -> Result<()> {
    let (command, exit_code, stderr) = match executor::load_last_exec() {
        Ok(last) => {
            if last.exit_code == 0 {
                ui::print_info(tr.last_succeeded);
                return Ok(());
            }
            (last.command, last.exit_code, last.stderr)
        }
        Err(_) => {
            ui::print_info(tr.no_piz_record);
            let cmd = history::last_history_command()?;
            ui::print_info(&format!("{} {}", tr.last_from_history, cmd));
            (cmd, 1, String::new())
        }
    };

    println!();
    ui::print_info(&format!("{} {}", tr.failed_command, command));
    if !stderr.is_empty() {
        println!(
            "  {}",
            stderr.lines().take(5).collect::<Vec<_>>().join("\n  ")
        );
    }
    println!();

    let (system, user) = build_fix_prompt(ctx, &command, exit_code, &stderr, lang);

    let spinner = ui::create_spinner(tr.analyzing);
    let response = backend.chat(&system, &user).await?;
    spinner.finish_and_clear();

    let (diagnosis, fixed_cmd, llm_danger) = parse_fix_response(&response)?;

    ui::print_diagnosis(tr, &diagnosis);
    println!();

    let regex_danger = danger::detect_danger_regex(&fixed_cmd);
    let final_danger = regex_danger.max(llm_danger);

    let choice = executor::prompt_user(&fixed_cmd, final_danger, false, tr, None)?;
    match choice {
        UserChoice::Execute => {
            let (code, _out, stderr_out) =
                executor::execute_command_with_shell(&fixed_cmd, &ctx.shell, tr)?;
            if code != 0 {
                try_auto_fix(&fixed_cmd, code, &stderr_out, tr, backend, ctx, lang).await?;
            }
        }
        UserChoice::Edit(edited) => {
            let (code, _out, stderr_out) =
                executor::execute_command_with_shell(&edited, &ctx.shell, tr)?;
            if code != 0 {
                try_auto_fix(&edited, code, &stderr_out, tr, backend, ctx, lang).await?;
            }
        }
        UserChoice::Cancel => {
            ui::print_info(tr.cancelled);
        }
    }

    Ok(())
}

pub async fn try_auto_fix(
    failed_cmd: &str,
    exit_code: i32,
    stderr: &str,
    tr: &i18n::T,
    backend: &dyn LlmBackend,
    ctx: &SystemContext,
    lang: &str,
) -> Result<()> {
    println!();
    let do_fix = dialoguer::Confirm::new()
        .with_prompt(tr.auto_fix_prompt)
        .default(true)
        .interact()?;

    if !do_fix {
        return Ok(());
    }

    let mut current_cmd = failed_cmd.to_string();
    let mut current_stderr = stderr.to_string();
    let mut current_exit_code = exit_code;

    for attempt in 1..=MAX_AUTO_FIX_RETRIES {
        let spinner = ui::create_spinner(tr.auto_fix_attempting);
        let (system, user) =
            build_fix_prompt(ctx, &current_cmd, current_exit_code, &current_stderr, lang);
        let response = backend.chat(&system, &user).await?;
        spinner.finish_and_clear();

        let (diagnosis, fixed_cmd, llm_danger) = match parse_fix_response(&response) {
            Ok(r) => r,
            Err(e) => {
                ui::print_error(&format!("{} {}", tr.auto_fix_failed, e));
                return Ok(());
            }
        };

        ui::print_diagnosis(tr, &diagnosis);
        ui::print_command_diff(&current_cmd, &fixed_cmd);
        println!();

        // Injection check on fix
        if let Some(reason) = danger::detect_injection(&fixed_cmd) {
            ui::print_error(&format!("{} {}", tr.auto_fix_failed, reason.message(tr)));
            return Ok(());
        }

        let regex_danger = danger::detect_danger_regex(&fixed_cmd);
        let final_danger = regex_danger.max(llm_danger);

        let choice = executor::prompt_user(&fixed_cmd, final_danger, false, tr, None)?;
        match choice {
            UserChoice::Execute => {
                let (code, _out, err) =
                    executor::execute_command_with_shell(&fixed_cmd, &ctx.shell, tr)?;
                if code == 0 {
                    return Ok(());
                }
                current_cmd = fixed_cmd;
                current_stderr = err;
                current_exit_code = code;

                if attempt < MAX_AUTO_FIX_RETRIES {
                    println!();
                    ui::print_info(&format!(
                        "{} ({}/{})",
                        tr.auto_fix_attempting,
                        attempt + 1,
                        MAX_AUTO_FIX_RETRIES
                    ));
                }
            }
            UserChoice::Edit(edited) => {
                if let Some(reason) = danger::detect_injection(&edited) {
                    ui::print_error(&format!("{} {}", tr.auto_fix_failed, reason.message(tr)));
                    return Ok(());
                }
                let (code, _out, err) =
                    executor::execute_command_with_shell(&edited, &ctx.shell, tr)?;
                if code == 0 {
                    return Ok(());
                }
                current_cmd = edited;
                current_stderr = err;
                current_exit_code = code;

                if attempt < MAX_AUTO_FIX_RETRIES {
                    println!();
                    ui::print_info(&format!(
                        "{} ({}/{})",
                        tr.auto_fix_attempting,
                        attempt + 1,
                        MAX_AUTO_FIX_RETRIES
                    ));
                }
            }
            UserChoice::Cancel => {
                return Ok(());
            }
        }
    }

    ui::print_error(&format!(
        "{} reached max retries ({})",
        tr.auto_fix_failed, MAX_AUTO_FIX_RETRIES
    ));
    Ok(())
}

pub fn parse_fix_response(response: &str) -> Result<(String, String, DangerLevel)> {
    let trimmed = response.trim();

    // Level 1: Direct JSON parse
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return extract_fix_json(&v);
    }

    // Level 2: Find JSON block in text
    if let Some(json_str) = crate::extract_json_block(trimmed) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            return extract_fix_json(&v);
        }
    }

    // Level 3: Structural regex extraction for broken JSON (e.g. unescaped Windows paths)
    if let Some(result) = extract_fix_by_structure(trimmed) {
        return Ok(result);
    }

    anyhow::bail!("Could not parse fix response from LLM:\n{}", response)
}

/// Extract fix fields from malformed JSON using structural regex patterns.
/// Handles common case of unescaped backslashes in Windows paths.
fn extract_fix_by_structure(text: &str) -> Option<(String, String, DangerLevel)> {
    // Extract diagnosis
    let re_diag = regex::Regex::new(r#""diagnosis"\s*:\s*"(.*?)"\s*[,}]"#).ok()?;
    let diagnosis = re_diag
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "Unknown issue".to_string());

    // Extract command — use "danger" field as right boundary to handle unescaped backslashes
    let re_cmd = regex::Regex::new(
        r#""command"\s*:\s*"(.*?)"\s*[,}]\s*"danger"\s*:\s*"(safe|warning|dangerous)""#,
    )
    .ok()?;

    if let Some(caps) = re_cmd.captures(text) {
        let cmd = caps.get(1)?.as_str().to_string();
        let danger_str = caps.get(2)?.as_str();
        let danger = DangerLevel::from_str_level(danger_str);
        if !cmd.is_empty() {
            return Some((diagnosis, cmd, danger));
        }
    }

    None
}

fn extract_fix_json(v: &serde_json::Value) -> Result<(String, String, DangerLevel)> {
    let diagnosis = v["diagnosis"]
        .as_str()
        .unwrap_or("Unknown issue")
        .to_string();
    let command = v["command"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No 'command' field in fix response"))?
        .to_string();
    let danger = v["danger"]
        .as_str()
        .map(DangerLevel::from_str_level)
        .unwrap_or(DangerLevel::Safe);
    Ok((diagnosis, command, danger))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fix_direct_json() {
        let input = r#"{"diagnosis": "Permission denied", "command": "sudo npm install", "danger": "warning"}"#;
        let (diag, cmd, danger) = parse_fix_response(input).unwrap();
        assert_eq!(diag, "Permission denied");
        assert_eq!(cmd, "sudo npm install");
        assert_eq!(danger, DangerLevel::Warning);
    }

    #[test]
    fn parse_fix_json_in_text() {
        let input = r#"The fix is: {"diagnosis": "wrong path", "command": "cd /correct/path && npm start", "danger": "safe"} Try this."#;
        let (diag, cmd, danger) = parse_fix_response(input).unwrap();
        assert_eq!(diag, "wrong path");
        assert!(cmd.contains("cd /correct/path"));
        assert_eq!(danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_fix_missing_diagnosis_defaults() {
        let input = r#"{"command": "npm install"}"#;
        let (diag, cmd, _) = parse_fix_response(input).unwrap();
        assert_eq!(diag, "Unknown issue");
        assert_eq!(cmd, "npm install");
    }

    #[test]
    fn parse_fix_missing_danger_defaults_safe() {
        let input = r#"{"diagnosis": "typo", "command": "git push"}"#;
        let (_, _, danger) = parse_fix_response(input).unwrap();
        assert_eq!(danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_fix_no_command_field_errors() {
        let input = r#"{"diagnosis": "something wrong"}"#;
        assert!(parse_fix_response(input).is_err());
    }

    #[test]
    fn parse_fix_invalid_text_errors() {
        let input = "I'm not sure what went wrong.";
        assert!(parse_fix_response(input).is_err());
    }

    #[test]
    fn parse_fix_dangerous_command() {
        let input =
            r#"{"diagnosis": "need root", "command": "rm -rf /tmp/*", "danger": "dangerous"}"#;
        let (_, _, danger) = parse_fix_response(input).unwrap();
        assert_eq!(danger, DangerLevel::Dangerous);
    }

    // ── Broken JSON (Windows paths) ──

    #[test]
    fn parse_fix_broken_json_windows_path() {
        let input = r#"{"diagnosis": "路径错误", "command": "cd /d D:\", "danger": "safe"}"#;
        let (diag, cmd, danger) = parse_fix_response(input).unwrap();
        assert_eq!(diag, "路径错误");
        assert_eq!(cmd, r#"cd /d D:\"#);
        assert_eq!(danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_fix_broken_json_windows_path_with_subdir() {
        let input =
            r#"{"diagnosis": "wrong dir", "command": "dir C:\Users\test", "danger": "safe"}"#;
        let (_, cmd, _) = parse_fix_response(input).unwrap();
        assert!(cmd.contains(r"C:\Users\test"));
    }
}

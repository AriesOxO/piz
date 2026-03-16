mod cache;
mod chat;
mod cli;
mod config;
mod context;
mod danger;
mod executor;
mod explain;
mod fix;
mod history;
mod i18n;
mod llm;
mod ui;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::danger::DangerLevel;
use crate::executor::UserChoice;
use crate::i18n::Lang;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        ui::print_error(&format!("{:#}", e));
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    if std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    let cli = Cli::parse();

    // Handle completions before loading config (no config needed)
    if let Some(Commands::Completions { shell }) = &cli.command {
        Cli::generate_completions(*shell);
        return Ok(());
    }

    // Handle config subcommand before loading config
    if let Some(Commands::Config { init, show, reset }) = &cli.command {
        if *init {
            return config::init_config();
        }
        if *reset {
            let path = config::config_path()?;
            if path.exists() {
                let confirm = dialoguer::Confirm::new()
                    .with_prompt(format!("Delete {}?", path.display()))
                    .default(false)
                    .interact()?;
                if confirm {
                    std::fs::remove_file(&path)?;
                    println!("Config reset. Run `piz config --init` to reconfigure.");
                }
            } else {
                println!("No config file found.");
            }
            return Ok(());
        }
        if *show {
            let path = config::config_path()?;
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                let masked = config::mask_config_keys(&content);
                println!("Config path: {}", path.display());
                println!("{}", masked);
            } else {
                println!("No config file found. Run `piz config --init` to create one.");
            }
            return Ok(());
        }
        let path = config::config_path()?;
        println!("Config path: {}", path.display());
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            println!("{}", content);
        }
        return Ok(());
    }

    // Load config (auto-triggers init wizard if missing)
    let cfg = config::load_config()?;
    let lang = Lang::from_code(&cfg.language);
    let tr = i18n::t(lang);

    // Handle remaining subcommands
    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::ClearCache => {
                let c = cache::Cache::open(cfg.cache_ttl_hours)?;
                let count = c.clear()?;
                println!("Cleared {} cached entries.", count);
                return Ok(());
            }
            Commands::Fix => {
                let ctx = context::collect_context();
                let backend = llm::create_backend(&cfg, cli.backend.as_deref())?;
                return fix::fix_last_command(backend.as_ref(), &ctx, tr, lang.code()).await;
            }
            Commands::Chat => {
                let ctx = context::collect_context();
                let backend = llm::create_backend(&cfg, cli.backend.as_deref())?;
                return chat::run_chat(
                    backend.as_ref(),
                    &ctx,
                    tr,
                    lang.code(),
                    cfg.auto_confirm_safe,
                    cfg.chat_history_size,
                    cli.verbose,
                )
                .await;
            }
            Commands::History { search, limit } => {
                let c = cache::Cache::open_with_max(cfg.cache_ttl_hours, cfg.cache_max_entries)?;
                let entries = if let Some(pattern) = search {
                    c.search_history(pattern, *limit)?
                } else {
                    c.list_history(*limit)?
                };
                if entries.is_empty() {
                    println!("No history found.");
                } else {
                    for (query, command, exit_code, danger, _ts) in &entries {
                        let status = if *exit_code == 0 { "✓" } else { "✗" };
                        println!("  {} [{}] {} → {}", status, danger, query, command);
                    }
                }
                return Ok(());
            }
            Commands::Config { .. } => unreachable!("Config handled earlier"),
            Commands::Completions { .. } => unreachable!("Completions handled earlier"),
        }
    }

    // Handle explain mode
    if let Some(cmd_to_explain) = &cli.explain {
        let ctx = context::collect_context();
        let backend = llm::create_backend(&cfg, cli.backend.as_deref())?;
        return explain::explain_command(backend.as_ref(), &ctx, cmd_to_explain, tr, lang.code())
            .await;
    }

    // Main flow: natural language → command
    let mut query = cli.query.join(" ");

    // In pipe mode, read from stdin if no query args
    if cli.pipe && query.is_empty() {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
        query = buf.trim().to_string();
    }

    if query.is_empty() {
        Cli::parse_from(["piz", "--help"]);
        return Ok(());
    }

    let ctx = context::collect_context();

    // Create backend early (needed for both cache hits and LLM calls, for auto-fix)
    let backend = llm::create_backend(&cfg, cli.backend.as_deref())?;

    // Open cache once and reuse
    let cache = if !cli.no_cache {
        Some(cache::Cache::open_with_max(
            cfg.cache_ttl_hours,
            cfg.cache_max_entries,
        )?)
    } else {
        None
    };

    // Check cache
    if let Some(ref c) = cache {
        if let Some((cached_cmd, cached_danger)) = c.get(&query, &ctx.os, &ctx.shell)? {
            // Re-validate injection detection on cached commands
            if let Some(reason) = danger::detect_injection(&cached_cmd) {
                ui::print_danger(tr);
                ui::print_info(reason.message(tr));
                let _ = c.delete(&query, &ctx.os, &ctx.shell);
                anyhow::bail!("Cached command blocked: {}", reason.message(tr));
            }

            // Pipe mode: output only the command
            if cli.pipe {
                println!("{}", cached_cmd);
                return Ok(());
            }

            ui::print_cached(tr);
            let regex_danger = danger::detect_danger_regex(&cached_cmd);
            let llm_danger = DangerLevel::from_str_level(&cached_danger);
            let final_danger = regex_danger.max(llm_danger);

            return handle_command_with_autofix(
                &cached_cmd,
                final_danger,
                cfg.auto_confirm_safe,
                tr,
                backend.as_ref(),
                &ctx,
                lang.code(),
            )
            .await;
        }
    }

    // Call LLM (with implicit context from last execution)
    let prev_context = executor::load_last_exec()
        .ok()
        .map(|last| llm::prompt::PrevContext {
            command: last.command,
            exit_code: last.exit_code,
            stdout_preview: last.stdout.lines().take(3).collect::<Vec<_>>().join("\n"),
        });
    let (system_prompt, user_prompt) = llm::prompt::build_translate_prompt_with_context(
        &ctx,
        &query,
        lang.code(),
        prev_context.as_ref(),
    );

    if cli.verbose {
        eprintln!("[verbose] system prompt length: {} chars", system_prompt.len());
        eprintln!("[verbose] user prompt: {}", user_prompt);
    }

    let response = if cli.pipe {
        backend.chat(&system_prompt, &user_prompt).await?
    } else {
        let spinner = ui::create_spinner(tr.thinking);
        let r = backend.chat(&system_prompt, &user_prompt).await?;
        spinner.finish_and_clear();
        r
    };

    if cli.verbose {
        eprintln!("[verbose] response: {}", response);
    }

    // Parse response (4-level fallback)
    let (command, llm_danger) = parse_llm_response(&response)?;

    // Injection detection
    if let Some(reason) = danger::detect_injection(&command) {
        if !cli.pipe {
            ui::print_danger(tr);
            ui::print_info(reason.message(tr));
        }
        anyhow::bail!("Command blocked: {}", reason.message(tr));
    }

    // Pipe mode: output only the command
    if cli.pipe {
        println!("{}", command);
        return Ok(());
    }

    // Danger detection: regex + LLM
    let regex_danger = danger::detect_danger_regex(&command);
    let final_danger = regex_danger.max(llm_danger);

    // Cache the result
    if let Some(ref c) = cache {
        let _ = c.put(&query, &ctx.os, &ctx.shell, &command, final_danger.as_str());
    }

    let result = handle_command_with_autofix(
        &command,
        final_danger,
        cfg.auto_confirm_safe,
        tr,
        backend.as_ref(),
        &ctx,
        lang.code(),
    )
    .await;

    // Record execution in history
    if let Some(ref c) = cache {
        // Load last exec to get exit code
        if let Ok(last) = executor::load_last_exec() {
            let _ = c.record_execution(
                &query,
                &last.command,
                last.exit_code,
                final_danger.as_str(),
            );
        }
    }

    result
}

async fn handle_command_with_autofix(
    command: &str,
    danger: DangerLevel,
    auto_confirm: bool,
    tr: &i18n::T,
    backend: &dyn llm::LlmBackend,
    ctx: &context::SystemContext,
    lang: &str,
) -> Result<()> {
    let choice = executor::prompt_user(command, danger, auto_confirm, tr)?;
    let exec_result = match choice {
        UserChoice::Execute => Some(executor::execute_command(command, tr)?),
        UserChoice::Edit(ref edited) => {
            // Re-check edited command for injection and danger
            if let Some(reason) = danger::detect_injection(edited) {
                ui::print_danger(tr);
                ui::print_info(reason.message(tr));
                anyhow::bail!("Edited command blocked: {}", reason.message(tr));
            }
            Some(executor::execute_command(edited, tr)?)
        }
        UserChoice::Cancel => {
            ui::print_info(tr.cancelled);
            return Ok(());
        }
    };

    // Auto-fix if command failed
    if let Some((exit_code, _stdout, stderr)) = exec_result {
        if exit_code != 0 {
            let executed_cmd = match &choice {
                UserChoice::Edit(edited) => edited.as_str(),
                _ => command,
            };
            try_auto_fix(executed_cmd, exit_code, &stderr, tr, backend, ctx, lang).await?;
        }
    }

    Ok(())
}

/// Maximum auto-fix retry attempts
const MAX_AUTO_FIX_RETRIES: usize = 3;

async fn try_auto_fix(
    failed_cmd: &str,
    exit_code: i32,
    stderr: &str,
    tr: &i18n::T,
    backend: &dyn llm::LlmBackend,
    ctx: &context::SystemContext,
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
        let (system, user) = llm::prompt::build_fix_prompt(
            ctx,
            &current_cmd,
            current_exit_code,
            &current_stderr,
            lang,
        );
        let response = backend.chat(&system, &user).await?;
        spinner.finish_and_clear();

        let (diagnosis, fixed_cmd, llm_danger) = match fix::parse_fix_response(&response) {
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

        let choice = executor::prompt_user(&fixed_cmd, final_danger, false, tr)?;
        match choice {
            UserChoice::Execute => {
                let (code, _out, err) = executor::execute_command(&fixed_cmd, tr)?;
                if code == 0 {
                    return Ok(());
                }
                // Still failing — loop for next attempt
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
                // Re-check edited command for injection
                if let Some(reason) = danger::detect_injection(&edited) {
                    ui::print_error(&format!("{} {}", tr.auto_fix_failed, reason.message(tr)));
                    return Ok(());
                }
                let (code, _out, err) = executor::execute_command(&edited, tr)?;
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

/// Handle command in chat mode (non-fatal, continues the loop)
pub fn handle_command_in_chat(
    command: &str,
    danger: DangerLevel,
    auto_confirm: bool,
    tr: &i18n::T,
) {
    let choice = match executor::prompt_user(command, danger, auto_confirm, tr) {
        Ok(c) => c,
        Err(e) => {
            ui::print_error(&format!("{:#}", e));
            return;
        }
    };
    match choice {
        UserChoice::Execute => {
            if let Err(e) = executor::execute_command(command, tr) {
                ui::print_error(&format!("{:#}", e));
            }
        }
        UserChoice::Edit(edited) => {
            // Re-check edited command for injection
            if let Some(reason) = danger::detect_injection(&edited) {
                ui::print_danger(tr);
                ui::print_info(reason.message(tr));
                return;
            }
            if let Err(e) = executor::execute_command(&edited, tr) {
                ui::print_error(&format!("{:#}", e));
            }
        }
        UserChoice::Cancel => {
            ui::print_info(tr.cancelled);
        }
    }
}

/// Check if LLM response is a refusal (non-command input detected)
fn check_refusal(v: &serde_json::Value) -> Option<String> {
    if v["refuse"].as_bool() == Some(true) {
        let msg = v["message"].as_str().unwrap_or("Not a command request.");
        return Some(msg.to_string());
    }
    // Also refuse if command is empty
    if let Some(cmd) = v["command"].as_str() {
        if cmd.trim().is_empty() {
            let msg = v["message"].as_str().unwrap_or("No command generated.");
            return Some(msg.to_string());
        }
    }
    None
}

/// Parse LLM response with 4-level fallback:
/// 1. Direct JSON
/// 2. Extract JSON block from text
/// 3. Extract backtick-wrapped command
/// 4. Raw text as command
pub fn parse_llm_response(response: &str) -> Result<(String, DangerLevel)> {
    let trimmed = response.trim();

    // Level 1: Direct JSON parse
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(refused) = check_refusal(&v) {
            anyhow::bail!("{}", refused);
        }
        if let Some(cmd) = v["command"].as_str() {
            let danger = v["danger"]
                .as_str()
                .map(DangerLevel::from_str_level)
                .unwrap_or(DangerLevel::Safe);
            return Ok((cmd.to_string(), danger));
        }
    }

    // Level 2: Find JSON block in text (match braces properly)
    if let Some(json_str) = extract_json_block(trimmed) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(refused) = check_refusal(&v) {
                anyhow::bail!("{}", refused);
            }
            if let Some(cmd) = v["command"].as_str() {
                let danger = v["danger"]
                    .as_str()
                    .map(DangerLevel::from_str_level)
                    .unwrap_or(DangerLevel::Safe);
                return Ok((cmd.to_string(), danger));
            }
        }
    }

    // Level 3: Extract backtick-wrapped command (handle triple backticks)
    if let Some(cmd) = extract_backtick_command(trimmed) {
        if !cmd.is_empty() {
            return Ok((cmd.to_string(), DangerLevel::Safe));
        }
    }

    // Level 4: Use raw text (first non-empty line)
    let cmd = trimmed
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(trimmed)
        .trim()
        .to_string();

    if cmd.is_empty() {
        anyhow::bail!("LLM returned empty response");
    }

    Ok((cmd, DangerLevel::Safe))
}

/// Extract a JSON object from text by matching braces
pub fn extract_json_block(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;
    for (i, ch) in text[start..].char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract command from backtick-wrapped text, handling both single and triple backticks
fn extract_backtick_command(text: &str) -> Option<&str> {
    // Try triple backticks first (```...```)
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        // Skip optional language tag on the same line
        let content_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after[content_start..];
        if let Some(end) = content.find("```") {
            let cmd = content[..end].trim();
            if !cmd.is_empty() {
                return Some(cmd);
            }
        }
    }
    // Try single backticks
    if let Some(start) = text.find('`') {
        if let Some(end) = text[start + 1..].find('`') {
            let cmd = &text[start + 1..start + 1 + end];
            if !cmd.is_empty() {
                return Some(cmd);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Level 1: Direct JSON ──

    #[test]
    fn parse_direct_json_safe() {
        let input = r#"{"command": "ls -la", "danger": "safe"}"#;
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "ls -la");
        assert_eq!(danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_direct_json_dangerous() {
        let input = r#"{"command": "rm -rf /", "danger": "dangerous"}"#;
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "rm -rf /");
        assert_eq!(danger, DangerLevel::Dangerous);
    }

    #[test]
    fn parse_direct_json_warning() {
        let input = r#"{"command": "sudo apt update", "danger": "warning"}"#;
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "sudo apt update");
        assert_eq!(danger, DangerLevel::Warning);
    }

    #[test]
    fn parse_direct_json_missing_danger_defaults_safe() {
        let input = r#"{"command": "df -h"}"#;
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "df -h");
        assert_eq!(danger, DangerLevel::Safe);
    }

    // ── Level 2: JSON embedded in text ──

    #[test]
    fn parse_json_in_text() {
        let input =
            r#"Here is the command: {"command": "df -h", "danger": "safe"} Hope this helps!"#;
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "df -h");
        assert_eq!(danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_json_with_markdown_wrapper() {
        let input = "```json\n{\"command\": \"top -n 1\", \"danger\": \"safe\"}\n```";
        let (cmd, _) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "top -n 1");
    }

    // ── Level 3: Backtick-wrapped command ──

    #[test]
    fn parse_backtick_command() {
        let input = "You can use `du -sh *` to check sizes.";
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "du -sh *");
        assert_eq!(danger, DangerLevel::Safe);
    }

    // ── Level 4: Raw text ──

    #[test]
    fn parse_raw_text() {
        let input = "df -h";
        let (cmd, danger) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "df -h");
        assert_eq!(danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_raw_text_multiline_takes_first() {
        let input = "ls -la\nThis lists files";
        let (cmd, _) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "ls -la");
    }

    #[test]
    fn parse_raw_text_with_whitespace() {
        let input = "  \n  df -h  \n  ";
        let (cmd, _) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "df -h");
    }

    // ── Error case ──

    #[test]
    fn parse_empty_response_errors() {
        let result = parse_llm_response("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_whitespace_only_errors() {
        let result = parse_llm_response("   \n  \n  ");
        assert!(result.is_err());
    }

    // ── Edge cases ──

    #[test]
    fn parse_json_no_command_field_falls_through() {
        let input = r#"{"result": "ok"}"#;
        // No "command" field, should fall through to level 4 (raw text)
        let (cmd, _) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, r#"{"result": "ok"}"#);
    }

    #[test]
    fn parse_complex_command_in_json() {
        let input =
            r#"{"command": "find . -name '*.rs' -exec grep -l 'TODO' {} +", "danger": "safe"}"#;
        let (cmd, _) = parse_llm_response(input).unwrap();
        assert_eq!(cmd, "find . -name '*.rs' -exec grep -l 'TODO' {} +");
    }

    #[test]
    fn parse_pipe_command_in_json() {
        let input = r#"{"command": "ps aux | grep nginx | awk '{print $2}'", "danger": "safe"}"#;
        let (cmd, _) = parse_llm_response(input).unwrap();
        assert!(cmd.contains("ps aux"));
        assert!(cmd.contains("grep nginx"));
    }

    // ── Refusal detection ──

    #[test]
    fn parse_refusal_refuse_true() {
        let input =
            r#"{"command": "", "danger": "safe", "refuse": true, "message": "Not a command."}"#;
        let result = parse_llm_response(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("Not a command"));
    }

    #[test]
    fn parse_refusal_empty_command() {
        let input = r#"{"command": "", "danger": "safe"}"#;
        let result = parse_llm_response(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_refusal_embedded_in_text() {
        let input = r#"Sorry: {"command": "", "refuse": true, "message": "Greeting detected."}"#;
        let result = parse_llm_response(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_normal_command_not_refused() {
        let input = r#"{"command": "ls -la", "danger": "safe", "refuse": false}"#;
        let result = parse_llm_response(input);
        assert!(result.is_ok());
    }
}

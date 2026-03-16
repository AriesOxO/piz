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
    let cli = Cli::parse();

    // Handle config --init before loading config
    if let Some(Commands::Config { init }) = &cli.command {
        if *init {
            return config::init_config();
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
                )
                .await;
            }
            Commands::Config { .. } => unreachable!(),
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
    let query = cli.query.join(" ");
    if query.is_empty() {
        Cli::parse_from(["piz", "--help"]);
        return Ok(());
    }

    let ctx = context::collect_context();

    // Check cache
    if !cli.no_cache {
        let c = cache::Cache::open(cfg.cache_ttl_hours)?;
        if let Some((cached_cmd, cached_danger)) = c.get(&query, &ctx.os, &ctx.shell)? {
            ui::print_cached(tr);
            let regex_danger = danger::detect_danger_regex(&cached_cmd);
            let llm_danger = DangerLevel::from_str_level(&cached_danger);
            let final_danger = regex_danger.max(llm_danger);

            return handle_command(&cached_cmd, final_danger, cfg.auto_confirm_safe, tr);
        }
    }

    // Call LLM (with implicit context from last execution)
    let backend = llm::create_backend(&cfg, cli.backend.as_deref())?;
    let prev_context = executor::load_last_exec()
        .ok()
        .map(|last| llm::prompt::PrevContext {
            command: last.command,
            exit_code: last.exit_code,
            stdout_preview: last.stderr.lines().take(3).collect::<Vec<_>>().join("\n"),
        });
    let (system_prompt, user_prompt) = llm::prompt::build_translate_prompt_with_context(
        &ctx,
        &query,
        lang.code(),
        prev_context.as_ref(),
    );

    let spinner = ui::create_spinner(tr.thinking);
    let response = backend.chat(&system_prompt, &user_prompt).await?;
    spinner.finish_and_clear();

    // Parse response (4-level fallback)
    let (command, llm_danger) = parse_llm_response(&response)?;

    // Injection detection
    if let Some(reason) = danger::detect_injection(&command) {
        ui::print_danger(tr);
        ui::print_info(reason);
        anyhow::bail!("Command blocked: {}", reason);
    }

    // Danger detection: regex + LLM
    let regex_danger = danger::detect_danger_regex(&command);
    let final_danger = regex_danger.max(llm_danger);

    // Cache the result
    if !cli.no_cache {
        let c = cache::Cache::open(cfg.cache_ttl_hours)?;
        let danger_str = match final_danger {
            DangerLevel::Safe => "safe",
            DangerLevel::Warning => "warning",
            DangerLevel::Dangerous => "dangerous",
        };
        let _ = c.put(&query, &ctx.os, &ctx.shell, &command, danger_str);
    }

    handle_command(&command, final_danger, cfg.auto_confirm_safe, tr)
}

fn handle_command(
    command: &str,
    danger: DangerLevel,
    auto_confirm: bool,
    tr: &i18n::T,
) -> Result<()> {
    let choice = executor::prompt_user(command, danger, auto_confirm, tr)?;
    match choice {
        UserChoice::Execute => {
            executor::execute_command(command, tr)?;
        }
        UserChoice::Edit(edited) => {
            executor::execute_command(&edited, tr)?;
        }
        UserChoice::Cancel => {
            ui::print_info(tr.cancelled);
        }
    }
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
            let _ = executor::execute_command(command, tr);
        }
        UserChoice::Edit(edited) => {
            let _ = executor::execute_command(&edited, tr);
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

    // Level 2: Find JSON block in text
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
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
    }

    // Level 3: Extract backtick-wrapped command
    if let Some(start) = trimmed.find('`') {
        if let Some(end) = trimmed[start + 1..].find('`') {
            let cmd = &trimmed[start + 1..start + 1 + end];
            if !cmd.is_empty() {
                return Ok((cmd.to_string(), DangerLevel::Safe));
            }
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

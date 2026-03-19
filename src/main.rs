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
mod shell_init;
mod ui;
mod update;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::danger::DangerLevel;
use crate::executor::UserChoice;
use crate::i18n::Lang;

#[tokio::main]
async fn main() {
    enable_ansi_support();
    if let Err(e) = run().await {
        ui::print_error(&format!("{:#}", e));
        std::process::exit(1);
    }
}

/// Enable ANSI/VT100 escape sequence support on Windows.
/// Windows PowerShell 5.1 does not enable Virtual Terminal Processing by default,
/// causing raw escape codes (like `␛[36m`) to be displayed instead of colors.
/// VT support requires Windows 10 1511+; if SetConsoleMode fails (older Windows),
/// we fall back to disabling colors entirely.
fn enable_ansi_support() {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::Console::{
            GetConsoleMode, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        };
        // SAFETY: We obtain the stdout handle via AsRawHandle and pass it to
        // GetConsoleMode/SetConsoleMode. These are safe Windows API calls that only
        // read/write the console mode flags. The handle is valid for the lifetime of
        // stdout and we check return values before using results.
        unsafe {
            let handle =
                std::io::stdout().as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
            let mut mode: u32 = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                // Try to enable VT processing; if it fails (old Windows), disable colors
                if SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0 {
                    colored::control::set_override(false);
                }
            }
        }
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

    // Handle init before loading config (no config needed)
    if let Some(Commands::Init { shell }) = &cli.command {
        let code = shell_init::generate_init(shell)?;
        print!("{}", code);
        return Ok(());
    }

    // Handle update before loading config (no config needed)
    if let Some(Commands::Update) = &cli.command {
        // Load config for language if available, fallback to zh
        let lang = config::config_path()
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|c| toml::from_str::<config::Config>(&c).ok())
            .map(|c| Lang::from_code(&c.language))
            .unwrap_or(Lang::Zh);
        let tr = i18n::t(lang);
        return update::run_update(tr).await;
    }

    // Handle config subcommand before loading config
    if let Some(Commands::Config {
        init,
        show: _show,
        raw,
        reset,
    }) = &cli.command
    {
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
        if !*raw {
            let path = config::config_path()?;
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                println!("Config path: {}", path.display());
                println!("{}", config::mask_config_keys(&content));
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
                let chat_cfg = chat::ChatConfig {
                    backend: backend.as_ref(),
                    ctx: &ctx,
                    tr,
                    lang: lang.code(),
                    auto_confirm: cfg.auto_confirm_safe,
                    max_history: cfg.chat_history_size,
                    verbose: cli.verbose,
                    detail: cli.detail || cfg.show_explanation,
                };
                return chat::run_chat(&chat_cfg).await;
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
            Commands::Init { .. } => unreachable!("Init handled earlier"),
            Commands::Update => unreachable!("Update handled earlier"),
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

    let detail_active = cli.detail || cfg.show_explanation;

    // Check cache
    if let Some(ref c) = cache {
        if let Some((cached_cmd, cached_danger, cached_explanation)) =
            c.get(&query, &ctx.os, &ctx.shell)?
        {
            // Re-validate injection detection on cached commands
            if let Some(reason) = danger::detect_injection(&cached_cmd) {
                ui::print_danger(tr);
                ui::print_info(reason.message(tr));
                let _ = c.delete(&query, &ctx.os, &ctx.shell);
                anyhow::bail!("Cached command blocked: {}", reason.message(tr));
            }

            // Backfill explanation if detail mode is active but cache has none
            let explanation = if detail_active && cached_explanation.is_empty() {
                let spinner = ui::create_spinner(tr.detail_fetching);
                let (sys, usr) =
                    llm::prompt::build_detail_explain_prompt(&ctx, &cached_cmd, lang.code());
                let expl = match backend.chat(&sys, &usr).await {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("[warn] Failed to fetch explanation: {:#}", e);
                        String::new()
                    }
                };
                spinner.finish_and_clear();
                let _ = c.update_explanation(&query, &ctx.os, &ctx.shell, &expl);
                expl
            } else {
                cached_explanation
            };

            let explanation_ref = if detail_active && !explanation.is_empty() {
                Some(explanation.as_str())
            } else {
                None
            };

            // Pipe mode: output only the command
            if cli.pipe {
                if let Some(expl) = explanation_ref {
                    eprintln!("{}", expl);
                }
                println!("{}", cached_cmd);
                return Ok(());
            }

            ui::print_cached(tr);
            let regex_danger = danger::detect_danger_regex(&cached_cmd);
            let llm_danger = DangerLevel::from_str_level(&cached_danger);
            let final_danger = regex_danger.max(llm_danger);

            // Eval mode for cached commands
            if cli.eval {
                return handle_eval_mode(
                    &cached_cmd,
                    final_danger,
                    cfg.auto_confirm_safe,
                    tr,
                    explanation_ref,
                )
                .await;
            }

            let ec = ExecContext {
                backend: backend.as_ref(),
                ctx: &ctx,
                tr,
                lang: lang.code(),
                auto_confirm: cfg.auto_confirm_safe,
            };
            return handle_command_with_autofix(&cached_cmd, final_danger, &ec, explanation_ref)
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

    let candidates = cli.candidates.clamp(1, 5);

    let (system_prompt, user_prompt) = if candidates > 1 {
        llm::prompt::build_multi_candidate_prompt(
            &ctx,
            &query,
            lang.code(),
            candidates,
            prev_context.as_ref(),
        )
    } else {
        llm::prompt::build_translate_prompt_with_context(
            &ctx,
            &query,
            lang.code(),
            prev_context.as_ref(),
        )
    };

    // Augment prompt with explanation request if detail mode is active
    let system_prompt = if detail_active {
        llm::prompt::augment_prompt_with_explanation(&system_prompt, lang.code())
    } else {
        system_prompt
    };

    if cli.verbose {
        eprintln!(
            "[verbose] system prompt length: {} chars",
            system_prompt.len()
        );
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

    // Multi-candidate selection or single-command parsing
    let (command, llm_danger, explanation) = if candidates > 1 {
        select_from_candidates(&response, tr)?
    } else {
        let parsed = parse_llm_response(&response)?;
        (parsed.command, parsed.danger, parsed.explanation)
    };

    // Injection detection
    if let Some(reason) = danger::detect_injection(&command) {
        if !cli.pipe {
            ui::print_danger(tr);
            ui::print_info(reason.message(tr));
        }
        anyhow::bail!("Command blocked: {}", reason.message(tr));
    }

    let explanation_ref = if detail_active && !explanation.is_empty() {
        Some(explanation.as_str())
    } else {
        None
    };

    // Pipe mode: output only the command
    if cli.pipe {
        if let Some(expl) = explanation_ref {
            eprintln!("{}", expl);
        }
        println!("{}", command);
        return Ok(());
    }

    // Danger detection: regex + LLM
    let regex_danger = danger::detect_danger_regex(&command);
    let final_danger = regex_danger.max(llm_danger);

    // Eval mode: show UI, get confirmation, write command to file for shell wrapper
    if cli.eval {
        return handle_eval_mode(
            &command,
            final_danger,
            cfg.auto_confirm_safe,
            tr,
            explanation_ref,
        )
        .await;
    }

    // Execute the command (cache AFTER success, not before)
    let ec = ExecContext {
        backend: backend.as_ref(),
        ctx: &ctx,
        tr,
        lang: lang.code(),
        auto_confirm: cfg.auto_confirm_safe,
    };
    let result = handle_command_with_autofix(&command, final_danger, &ec, explanation_ref).await;

    // Record execution in history and cache only successful commands
    if let Some(ref c) = cache {
        if let Ok(last) = executor::load_last_exec() {
            let _ =
                c.record_execution(&query, &last.command, last.exit_code, final_danger.as_str());
            // Only cache commands that were successfully executed
            if last.exit_code == 0 {
                let _ = c.put(
                    &query,
                    &ctx.os,
                    &ctx.shell,
                    &command,
                    final_danger.as_str(),
                    &explanation,
                );
            }
        }
    }

    // Check for updates (at most once per 24h, fast: reads local state or 5s timeout)
    if !cli.pipe {
        update::check_update_hint().await;
    }

    result
}

/// Eval mode: show command and get user confirmation, then write to file for shell wrapper.
/// The shell wrapper function reads the file and evals the command in the current shell,
/// so cd, export, source, etc. all work correctly.
async fn handle_eval_mode(
    command: &str,
    danger: DangerLevel,
    auto_confirm: bool,
    tr: &i18n::T,
    explanation: Option<&str>,
) -> Result<()> {
    let choice = executor::prompt_user(command, danger, auto_confirm, tr, explanation)?;
    let final_cmd = match choice {
        UserChoice::Execute => command.to_string(),
        UserChoice::Edit(edited) => {
            if let Some(reason) = danger::detect_injection(&edited) {
                ui::print_danger(tr);
                ui::print_info(reason.message(tr));
                anyhow::bail!("Edited command blocked: {}", reason.message(tr));
            }
            edited
        }
        UserChoice::Cancel => {
            ui::print_info(tr.cancelled);
            // Remove eval file to signal cancellation
            let _ = std::fs::remove_file(config::piz_dir()?.join("eval_command"));
            std::process::exit(1);
        }
    };

    // Write command to file for shell wrapper to read and eval
    let dir = config::piz_dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("eval_command"), &final_cmd)?;

    Ok(())
}

/// Context for command execution with auto-fix support
struct ExecContext<'a> {
    backend: &'a dyn llm::LlmBackend,
    ctx: &'a context::SystemContext,
    tr: &'a i18n::T,
    lang: &'a str,
    auto_confirm: bool,
}

async fn handle_command_with_autofix(
    command: &str,
    danger: DangerLevel,
    ec: &ExecContext<'_>,
    explanation: Option<&str>,
) -> Result<()> {
    let tr = ec.tr;
    let auto_confirm = ec.auto_confirm;
    let choice = executor::prompt_user(command, danger, auto_confirm, tr, explanation)?;
    let exec_result = match choice {
        UserChoice::Execute => Some(executor::execute_command_with_shell(
            command,
            &ec.ctx.shell,
            tr,
        )?),
        UserChoice::Edit(ref edited) => {
            // Re-check edited command for injection and danger
            if let Some(reason) = danger::detect_injection(edited) {
                ui::print_danger(tr);
                ui::print_info(reason.message(tr));
                anyhow::bail!("Edited command blocked: {}", reason.message(tr));
            }
            Some(executor::execute_command_with_shell(
                edited,
                &ec.ctx.shell,
                tr,
            )?)
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
            fix::try_auto_fix(
                executed_cmd,
                exit_code,
                &stderr,
                tr,
                ec.backend,
                ec.ctx,
                ec.lang,
            )
            .await?;
        }
    }

    Ok(())
}

/// Result of parsing an LLM translate response
pub struct ParsedCommand {
    pub command: String,
    pub danger: DangerLevel,
    pub explanation: String,
}

/// A candidate command from multi-candidate response
struct Candidate {
    command: String,
    danger: DangerLevel,
    explanation: String,
}

/// Parse multi-candidate JSON array response, with fallback to single object
fn parse_multi_candidate_response(response: &str) -> Result<Vec<Candidate>> {
    let trimmed = response.trim();

    // Try parsing as JSON array
    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) {
        let candidates = extract_candidates_from_array(&arr);
        if !candidates.is_empty() {
            return Ok(candidates);
        }
    }

    // Try extracting JSON array from text
    if let Some(arr_str) = extract_json_array(trimmed) {
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(arr_str) {
            let candidates = extract_candidates_from_array(&arr);
            if !candidates.is_empty() {
                return Ok(candidates);
            }
        }
    }

    // Fallback: try parsing as single object and wrap
    let parsed = parse_llm_response(response)?;
    Ok(vec![Candidate {
        command: parsed.command,
        danger: parsed.danger,
        explanation: parsed.explanation,
    }])
}

fn extract_candidates_from_array(arr: &[serde_json::Value]) -> Vec<Candidate> {
    arr.iter()
        .filter_map(|v| {
            let cmd = v["command"].as_str()?.to_string();
            if cmd.is_empty() {
                return None;
            }
            let danger = v["danger"]
                .as_str()
                .map(DangerLevel::from_str_level)
                .unwrap_or(DangerLevel::Safe);
            let explanation = v["explanation"].as_str().unwrap_or("").to_string();
            Some(Candidate {
                command: cmd,
                danger,
                explanation,
            })
        })
        .collect()
}

/// Extract a JSON array from text by matching brackets
fn extract_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
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
            '[' if !in_string => depth += 1,
            ']' if !in_string => {
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

/// Present candidates to user via dialoguer::Select and return chosen command
fn select_from_candidates(response: &str, tr: &i18n::T) -> Result<(String, DangerLevel, String)> {
    let candidates = parse_multi_candidate_response(response)?;

    if candidates.len() == 1 {
        return Ok((
            candidates[0].command.clone(),
            candidates[0].danger,
            candidates[0].explanation.clone(),
        ));
    }

    let items: Vec<String> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if c.explanation.is_empty() {
                format!("{}. {}", i + 1, c.command)
            } else {
                format!("{}. {} — {}", i + 1, c.command, c.explanation)
            }
        })
        .collect();

    println!();
    let selection = dialoguer::Select::new()
        .with_prompt(tr.select_command)
        .items(&items)
        .default(0)
        .interact()?;

    let chosen = &candidates[selection];
    Ok((
        chosen.command.clone(),
        chosen.danger,
        chosen.explanation.clone(),
    ))
}

/// Handle command in chat mode (non-fatal, continues the loop)
pub fn handle_command_in_chat(
    command: &str,
    danger: DangerLevel,
    auto_confirm: bool,
    tr: &i18n::T,
    shell: &str,
    explanation: Option<&str>,
) {
    let choice = match executor::prompt_user(command, danger, auto_confirm, tr, explanation) {
        Ok(c) => c,
        Err(e) => {
            ui::print_error(&format!("{:#}", e));
            return;
        }
    };
    match choice {
        UserChoice::Execute => {
            if let Err(e) = executor::execute_command_with_shell(command, shell, tr) {
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
            if let Err(e) = executor::execute_command_with_shell(&edited, shell, tr) {
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

/// Parse LLM response with multi-level fallback:
/// 1. Direct JSON parse
/// 2. Extract JSON block from text
/// 3. Structural regex extraction (handles broken JSON escaping, e.g. Windows paths)
/// 4. Extract backtick-wrapped command (last resort)
///
/// NOTE: We intentionally do NOT fall back to "raw text as command" —
/// if all parsing levels fail, it's an error, not a command to execute.
pub fn parse_llm_response(response: &str) -> Result<ParsedCommand> {
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
            let explanation = v["explanation"].as_str().unwrap_or("").to_string();
            return Ok(ParsedCommand {
                command: cmd.to_string(),
                danger,
                explanation,
            });
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
                let explanation = v["explanation"].as_str().unwrap_or("").to_string();
                return Ok(ParsedCommand {
                    command: cmd.to_string(),
                    danger,
                    explanation,
                });
            }
        }
    }

    // Level 3: Structural regex extraction — handles broken JSON (e.g. unescaped backslashes)
    // Uses the known field structure to locate the command value without relying on JSON escaping.
    if let Some((cmd, danger)) = extract_command_by_structure(trimmed) {
        return Ok(ParsedCommand {
            command: cmd,
            danger,
            explanation: String::new(),
        });
    }

    // Level 4: Extract backtick-wrapped command (last resort for non-JSON responses)
    if let Some(cmd) = extract_backtick_command(trimmed) {
        if !cmd.is_empty() {
            return Ok(ParsedCommand {
                command: cmd.to_string(),
                danger: DangerLevel::Safe,
                explanation: String::new(),
            });
        }
    }

    // No more "raw text as command" — refuse to guess
    anyhow::bail!(
        "Failed to parse LLM response as a command. Raw response:\n{}",
        trimmed
    );
}

/// Extract command and danger from a malformed JSON response using structural patterns.
/// This handles the common case where LLM produces invalid JSON due to unescaped
/// backslashes in Windows paths (e.g. `"command": "cd D:\"` instead of `"cd D:\\"`).
fn extract_command_by_structure(text: &str) -> Option<(String, DangerLevel)> {
    // Check for refusal first — if refuse:true with empty command, return None
    // so the caller reports it as a parse error (which surfaces the refusal message).
    if text.contains("\"refuse\"") && text.contains("true") {
        if let Ok(re_cmd) = regex::Regex::new(r#""command"\s*:\s*"(.*?)""#) {
            if let Some(caps) = re_cmd.captures(text) {
                if caps
                    .get(1)
                    .map(|m| m.as_str().trim().is_empty())
                    .unwrap_or(true)
                {
                    return None;
                }
            }
        }
    }

    // Strategy: find "command": "..." followed by "danger": "..." using the structural
    // delimiter `, "danger"` or `,"danger"` to locate where the command value ends.
    // This avoids the ambiguity of `\"` being a JSON escape vs a Windows path separator.
    let re = regex::Regex::new(
        r#""command"\s*:\s*"(.*?)"\s*[,}]\s*"danger"\s*:\s*"(safe|warning|dangerous)""#,
    )
    .ok()?;

    if let Some(caps) = re.captures(text) {
        let cmd = caps.get(1)?.as_str().to_string();
        let danger_str = caps.get(2)?.as_str();
        let danger = DangerLevel::from_str_level(danger_str);
        if !cmd.is_empty() {
            return Some((cmd, danger));
        }
    }

    // Also try reversed field order: "danger" before "command"
    let re_rev = regex::Regex::new(
        r#""danger"\s*:\s*"(safe|warning|dangerous)"\s*[,}]\s*"command"\s*:\s*"(.*?)""#,
    )
    .ok()?;

    if let Some(caps) = re_rev.captures(text) {
        let danger_str = caps.get(1)?.as_str();
        let cmd = caps.get(2)?.as_str().to_string();
        let danger = DangerLevel::from_str_level(danger_str);
        if !cmd.is_empty() {
            return Some((cmd, danger));
        }
    }

    None
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
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "ls -la");
        assert_eq!(parsed.danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_direct_json_dangerous() {
        let input = r#"{"command": "rm -rf /", "danger": "dangerous"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "rm -rf /");
        assert_eq!(parsed.danger, DangerLevel::Dangerous);
    }

    #[test]
    fn parse_direct_json_warning() {
        let input = r#"{"command": "sudo apt update", "danger": "warning"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "sudo apt update");
        assert_eq!(parsed.danger, DangerLevel::Warning);
    }

    #[test]
    fn parse_direct_json_missing_danger_defaults_safe() {
        let input = r#"{"command": "df -h"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "df -h");
        assert_eq!(parsed.danger, DangerLevel::Safe);
    }

    // ── Level 2: JSON embedded in text ──

    #[test]
    fn parse_json_in_text() {
        let input =
            r#"Here is the command: {"command": "df -h", "danger": "safe"} Hope this helps!"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "df -h");
        assert_eq!(parsed.danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_json_with_markdown_wrapper() {
        let input = "```json\n{\"command\": \"top -n 1\", \"danger\": \"safe\"}\n```";
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "top -n 1");
    }

    // ── Level 3: Structural regex extraction (broken JSON) ──

    #[test]
    fn parse_broken_json_windows_path_backslash() {
        // LLM returns invalid JSON: D:\ causes the \" to be treated as escaped quote
        let input = r#"{"command": "cd /d D:\", "danger": "safe"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, r#"cd /d D:\"#);
        assert_eq!(parsed.danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_broken_json_windows_path_with_subdir() {
        let input = r#"{"command": "dir C:\Users\test", "danger": "safe"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert!(parsed.command.contains(r"C:\Users\test"));
        assert_eq!(parsed.danger, DangerLevel::Safe);
    }

    #[test]
    fn parse_broken_json_with_warning_danger() {
        let input = r#"{"command": "del C:\temp\*", "danger": "warning"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert!(parsed.command.contains(r"C:\temp\*"));
        assert_eq!(parsed.danger, DangerLevel::Warning);
    }

    // ── Level 4: Backtick-wrapped command ──

    #[test]
    fn parse_backtick_command() {
        let input = "You can use `du -sh *` to check sizes.";
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "du -sh *");
        assert_eq!(parsed.danger, DangerLevel::Safe);
    }

    // ── Error cases ──

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

    #[test]
    fn parse_raw_text_no_longer_accepted() {
        // Raw text should NOT be treated as a command — it's an error
        let result = parse_llm_response("df -h");
        assert!(result.is_err());
    }

    #[test]
    fn parse_json_no_command_field_errors() {
        let input = r#"{"result": "ok"}"#;
        // No "command" field, should be an error (no longer falls through to raw text)
        let result = parse_llm_response(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_complex_command_in_json() {
        let input =
            r#"{"command": "find . -name '*.rs' -exec grep -l 'TODO' {} +", "danger": "safe"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(
            parsed.command,
            "find . -name '*.rs' -exec grep -l 'TODO' {} +"
        );
    }

    #[test]
    fn parse_pipe_command_in_json() {
        let input = r#"{"command": "ps aux | grep nginx | awk '{print $2}'", "danger": "safe"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert!(parsed.command.contains("ps aux"));
        assert!(parsed.command.contains("grep nginx"));
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

    // ── Multi-candidate parsing ──

    #[test]
    fn parse_multi_candidate_json_array() {
        let input = r#"[{"command": "ls -la", "danger": "safe", "explanation": "List all files"},{"command": "find . -maxdepth 1", "danger": "safe", "explanation": "Find files in current dir"}]"#;
        let candidates = parse_multi_candidate_response(input).unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].command, "ls -la");
        assert_eq!(candidates[0].explanation, "List all files");
        assert_eq!(candidates[1].command, "find . -maxdepth 1");
    }

    #[test]
    fn parse_multi_candidate_fallback_to_single() {
        let input = r#"{"command": "ls -la", "danger": "safe"}"#;
        let candidates = parse_multi_candidate_response(input).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].command, "ls -la");
    }

    #[test]
    fn parse_multi_candidate_embedded_array() {
        let input = r#"Here are 3 options: [{"command":"ls","danger":"safe","explanation":"simple"},{"command":"dir","danger":"safe","explanation":"windows"}] Pick one."#;
        let candidates = parse_multi_candidate_response(input).unwrap();
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn parse_multi_candidate_skips_empty_commands() {
        let input = r#"[{"command": "ls", "danger": "safe"}, {"command": "", "danger": "safe"}]"#;
        let candidates = parse_multi_candidate_response(input).unwrap();
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn extract_json_array_basic() {
        let input = r#"text [{"a":1}] more"#;
        let arr = extract_json_array(input).unwrap();
        assert_eq!(arr, r#"[{"a":1}]"#);
    }

    #[test]
    fn extract_json_array_nested() {
        let input = r#"[{"a":[1,2]},{"b":3}]"#;
        let arr = extract_json_array(input).unwrap();
        assert_eq!(arr, input);
    }

    // ── Explanation parsing ──

    #[test]
    fn parse_response_with_explanation() {
        let input = r#"{"command": "ls -la", "danger": "safe", "explanation": "`ls` list files\n`-la` all + long"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "ls -la");
        assert_eq!(parsed.danger, DangerLevel::Safe);
        assert!(!parsed.explanation.is_empty());
        assert!(parsed.explanation.contains("list files"));
    }

    #[test]
    fn parse_response_without_explanation() {
        let input = r#"{"command": "ls -la", "danger": "safe"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, "ls -la");
        assert_eq!(parsed.explanation, "");
    }

    #[test]
    fn parse_structural_regex_has_empty_explanation() {
        // Broken JSON falls back to structural regex — explanation is lost, that's OK
        let input = r#"{"command": "cd /d D:\", "danger": "safe", "explanation": "changes dir"}"#;
        let parsed = parse_llm_response(input).unwrap();
        assert_eq!(parsed.command, r#"cd /d D:\"#);
        assert_eq!(parsed.explanation, "");
    }
}

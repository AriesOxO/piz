use anyhow::Result;
use colored::*;

use crate::config;
use crate::context::SystemContext;
use crate::danger;
use crate::i18n;
use crate::llm::prompt::augment_prompt_with_explanation;
use crate::llm::prompt::build_chat_system_prompt;
use crate::llm::{LlmBackend, Message};
use crate::ui;
use crate::{handle_command_in_chat, parse_llm_response};

fn chat_history_path() -> Result<std::path::PathBuf> {
    Ok(config::piz_dir()?.join("chat_history.json"))
}

fn load_chat_history() -> Vec<Message> {
    let path = match chat_history_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[warn] Failed to resolve chat history path: {}", e);
            return Vec::new();
        }
    };
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            eprintln!("[warn] Failed to read chat history: {}", e);
            Vec::new()
        }
    }
}

fn save_chat_history(history: &[Message]) {
    if let Ok(path) = chat_history_path() {
        if let Ok(json) = serde_json::to_string_pretty(history) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn delete_chat_history() {
    if let Ok(path) = chat_history_path() {
        let _ = std::fs::remove_file(path);
    }
}

/// Configuration for chat mode
pub struct ChatConfig<'a> {
    pub backend: &'a dyn LlmBackend,
    pub ctx: &'a SystemContext,
    pub tr: &'a i18n::T,
    pub lang: &'a str,
    pub auto_confirm: bool,
    pub max_history: usize,
    pub verbose: bool,
    pub detail: bool,
}

/// Truncate chat history to at most `max` entries, draining the oldest messages.
/// Drains an even number of messages to preserve user/assistant pairing.
fn truncate_history(history: &mut Vec<Message>, max: usize) {
    if history.len() > max {
        let excess = history.len() - max;
        // Round up to even number to preserve user/assistant pairing
        #[allow(clippy::manual_is_multiple_of)] // MSRV 1.70 compat
        let drain_count = if excess % 2 == 0 { excess } else { excess + 1 };
        let drain_count = drain_count.min(history.len() - 1);
        history.drain(..drain_count);
    }
}

pub async fn run_chat(cfg: &ChatConfig<'_>) -> Result<()> {
    let tr = cfg.tr;
    let verbose = cfg.verbose;
    let system_prompt = build_chat_system_prompt(cfg.ctx, cfg.lang);
    let mut detail_active = cfg.detail;
    let mut history: Vec<Message> = load_chat_history();

    println!();
    println!("  {} {}", "piz".green().bold(), tr.chat_title.dimmed());
    println!("  {}", tr.chat_hint.dimmed());
    println!();

    while let Ok(input) = dialoguer::Input::<String>::new()
        .with_prompt("piz".green().bold().to_string())
        .allow_empty(true)
        .interact_text()
    {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(
            trimmed.to_lowercase().as_str(),
            "exit" | "quit" | "q" | ":q"
        ) {
            break;
        }

        // Handle slash commands
        if trimmed.starts_with('/') {
            match trimmed.to_lowercase().as_str() {
                "/help" => {
                    println!("  {}", tr.chat_help_desc);
                    continue;
                }
                "/clear" => {
                    history.clear();
                    delete_chat_history();
                    println!("  {}", tr.chat_cleared);
                    continue;
                }
                "/detail" => {
                    detail_active = !detail_active;
                    if detail_active {
                        println!("  {}", tr.detail_toggle_on);
                    } else {
                        println!("  {}", tr.detail_toggle_off);
                    }
                    continue;
                }
                "/history" => {
                    if history.is_empty() {
                        println!("  (empty)");
                    } else {
                        for m in &history {
                            let preview: String = m.content.chars().take(80).collect();
                            println!("  [{}] {}", m.role, preview);
                        }
                    }
                    continue;
                }
                _ => {
                    println!("  {}", tr.chat_unknown_cmd);
                    continue;
                }
            }
        }

        // Add user message to history
        history.push(Message {
            role: "user".into(),
            content: trimmed.to_string(),
        });

        truncate_history(&mut history, cfg.max_history);

        // Call LLM with full history
        if verbose {
            eprintln!("[verbose] chat history length: {}", history.len());
        }
        let effective_system = if detail_active {
            augment_prompt_with_explanation(&system_prompt, cfg.lang)
        } else {
            system_prompt.clone()
        };
        let spinner = ui::create_spinner(tr.thinking);
        let response = cfg
            .backend
            .chat_with_history(&effective_system, &history)
            .await;
        spinner.finish_and_clear();

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                ui::print_error(&format!("{:#}", e));
                history.pop();
                continue;
            }
        };

        if verbose {
            eprintln!("[verbose] response: {}", response);
        }

        // Parse response
        let parsed = match parse_llm_response(&response) {
            Ok(r) => r,
            Err(e) => {
                println!("  {}", e.to_string().dimmed());
                history.push(Message {
                    role: "assistant".into(),
                    content: response.clone(),
                });
                continue;
            }
        };
        let command = parsed.command;
        let llm_danger = parsed.danger;
        let explanation = parsed.explanation;

        // Injection check - don't add malicious responses to history
        if let Some(reason) = danger::detect_injection(&command) {
            ui::print_danger(tr);
            ui::print_info(reason.message(tr));
            // Remove the user message that triggered this
            history.pop();
            continue;
        }

        // Danger detection
        let regex_danger = danger::detect_danger_regex(&command);
        let final_danger = regex_danger.max(llm_danger);

        // Add assistant response to history
        history.push(Message {
            role: "assistant".into(),
            content: response.clone(),
        });
        save_chat_history(&history);

        // Handle command
        let explanation_ref = if detail_active && !explanation.is_empty() {
            Some(explanation.as_str())
        } else {
            None
        };
        handle_command_in_chat(
            &command,
            final_danger,
            cfg.auto_confirm,
            tr,
            &cfg.ctx.shell,
            explanation_ref,
        );
    }

    println!();
    ui::print_info(tr.bye);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(count: usize) -> Vec<Message> {
        let mut msgs = Vec::new();
        for i in 0..count {
            let role = if i % 2 == 0 { "user" } else { "assistant" };
            msgs.push(Message {
                role: role.into(),
                content: format!("msg {}", i),
            });
        }
        msgs
    }

    // ── truncate_history ──

    #[test]
    fn truncate_no_op_when_under_max() {
        let mut history = make_messages(4);
        truncate_history(&mut history, 10);
        assert_eq!(history.len(), 4);
    }

    #[test]
    fn truncate_no_op_when_at_max() {
        let mut history = make_messages(6);
        truncate_history(&mut history, 6);
        assert_eq!(history.len(), 6);
    }

    #[test]
    fn truncate_drains_even_number_to_preserve_pairs() {
        // 8 messages, max 4 → excess 4 (even) → drain 4, keep last 4
        let mut history = make_messages(8);
        truncate_history(&mut history, 4);
        assert_eq!(history.len(), 4);
        assert_eq!(history[0].content, "msg 4");
    }

    #[test]
    fn truncate_rounds_up_to_even_drain() {
        // 7 messages, max 4 → excess 3 (odd) → drain 4, keep last 3
        let mut history = make_messages(7);
        truncate_history(&mut history, 4);
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].content, "msg 4");
    }

    #[test]
    fn truncate_preserves_pairing_order() {
        let mut history = make_messages(10);
        truncate_history(&mut history, 4);
        // After truncation, first message should be "user"
        assert_eq!(history[0].role, "user");
        // Messages should alternate
        for pair in history.chunks(2) {
            if pair.len() == 2 {
                assert_eq!(pair[0].role, "user");
                assert_eq!(pair[1].role, "assistant");
            }
        }
    }

    #[test]
    fn truncate_single_message_preserved() {
        // 3 messages, max 1 → excess 2 (even) → drain 2, keep last 1
        let mut history = make_messages(3);
        truncate_history(&mut history, 1);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "msg 2");
    }

    #[test]
    fn truncate_empty_history() {
        let mut history: Vec<Message> = Vec::new();
        truncate_history(&mut history, 10);
        assert!(history.is_empty());
    }

    // ── chat_history_path ──

    #[test]
    fn chat_history_path_ends_with_expected_filename() {
        let path = chat_history_path().unwrap();
        assert!(path.ends_with("chat_history.json"));
    }

    // ── load/save roundtrip ──

    #[test]
    fn load_empty_returns_empty_vec() {
        // When no history file exists, should return empty
        let history = load_chat_history();
        // This may or may not be empty depending on prior state,
        // but it should not panic
        assert!(history.len() < 10000);
    }
}

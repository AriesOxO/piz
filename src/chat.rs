use anyhow::Result;
use colored::*;

use crate::context::SystemContext;
use crate::danger;
use crate::i18n;
use crate::llm::prompt::build_chat_system_prompt;
use crate::llm::{LlmBackend, Message};
use crate::ui;
use crate::{handle_command_in_chat, parse_llm_response};

pub async fn run_chat(
    backend: &dyn LlmBackend,
    ctx: &SystemContext,
    tr: &i18n::T,
    lang: &str,
    auto_confirm: bool,
    max_history: usize,
    verbose: bool,
) -> Result<()> {
    let system_prompt = build_chat_system_prompt(ctx, lang);
    let mut history: Vec<Message> = Vec::new();

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

        // Add user message to history
        history.push(Message {
            role: "user".into(),
            content: trimmed.to_string(),
        });

        // Truncate history if too long, ensuring we keep pairs (drain even number)
        if history.len() > max_history {
            let excess = history.len() - max_history;
            // Round up to even number to preserve user/assistant pairing
            let drain_count = if excess % 2 == 0 { excess } else { excess + 1 };
            let drain_count = drain_count.min(history.len() - 1);
            history.drain(..drain_count);
        }

        // Call LLM with full history
        if verbose {
            eprintln!("[verbose] chat history length: {}", history.len());
        }
        let spinner = ui::create_spinner(tr.thinking);
        let response = backend.chat_with_history(&system_prompt, &history).await;
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
        let (command, llm_danger) = match parse_llm_response(&response) {
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

        // Handle command
        handle_command_in_chat(&command, final_danger, auto_confirm, tr);
    }

    println!();
    ui::print_info(tr.bye);
    Ok(())
}

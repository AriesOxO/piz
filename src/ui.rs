use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::i18n;

pub fn print_command(cmd: &str) {
    println!("  {} {}", "➜".green().bold(), cmd.white().bold());
}

pub fn print_warning(tr: &i18n::T) {
    println!("{} {}", "⚠".yellow().bold(), tr.modify_warning.yellow());
}

pub fn print_danger(tr: &i18n::T) {
    println!("{} {}", "🚨".red().bold(), tr.danger_warning.red().bold());
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}", "Error:".red().bold(), msg);
}

pub fn print_info(msg: &str) {
    println!("{} {}", "ℹ".blue(), msg);
}

pub fn print_cached(tr: &i18n::T) {
    println!("{}", tr.cached.dimmed());
}

pub fn print_explanation(tr: &i18n::T, text: &str) {
    println!("{} {}", "📖".green(), tr.command_explanation.green().bold());
    println!();
    for line in text.lines() {
        println!("  {}", line);
    }
    println!();
}

pub fn print_diagnosis(tr: &i18n::T, diagnosis: &str) {
    println!("{} {}", "🔧".yellow(), tr.diagnosis.yellow().bold());
    println!("  {}", diagnosis);
}

pub fn print_command_diff(original: &str, fixed: &str) {
    if original == fixed || original.is_empty() || fixed.is_empty() {
        return;
    }
    println!("  {} {}", "-".red(), original.red().strikethrough());
    println!("  {} {}", "+".green(), fixed.green().bold());
}

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_command_diff_same_noop() {
        // same commands should not panic
        print_command_diff("ls", "ls");
    }

    #[test]
    fn print_command_diff_empty_original_noop() {
        print_command_diff("", "ls");
    }

    #[test]
    fn print_command_diff_empty_fixed_noop() {
        print_command_diff("ls", "");
    }

    #[test]
    fn print_command_diff_both_empty_noop() {
        print_command_diff("", "");
    }

    #[test]
    fn print_command_diff_different_no_panic() {
        print_command_diff("ls -la", "ls -lh");
    }

    #[test]
    fn create_spinner_does_not_panic() {
        let spinner = create_spinner("loading...");
        spinner.finish_and_clear();
    }

    #[test]
    fn print_functions_do_not_panic() {
        let tr = crate::i18n::t(crate::i18n::Lang::En);
        print_command("echo hello");
        print_warning(tr);
        print_danger(tr);
        print_error("test error");
        print_info("test info");
        print_cached(tr);
        print_explanation(tr, "test explanation\nline 2");
        print_diagnosis(tr, "test diagnosis");
    }
}

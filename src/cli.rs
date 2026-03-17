use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "piz",
    version,
    about = "Intelligent terminal command assistant"
)]
pub struct Cli {
    /// Natural language description of the command you want
    pub query: Vec<String>,

    /// Explain a command instead of generating one
    #[arg(short = 'e', long = "explain")]
    pub explain: Option<String>,

    /// LLM backend to use (openai, claude, gemini, ollama)
    #[arg(short, long)]
    pub backend: Option<String>,

    /// Skip cache lookup
    #[arg(long)]
    pub no_cache: bool,

    /// Show debug info (prompts and LLM responses)
    #[arg(long)]
    pub verbose: bool,

    /// Pipe mode: output only the command, no UI
    #[arg(long)]
    pub pipe: bool,

    /// Eval mode: show UI normally, output confirmed command for shell wrapper to eval
    #[arg(long)]
    pub eval: bool,

    /// Number of candidate commands to generate (1-5)
    #[arg(short = 'n', long, default_value = "1")]
    pub candidates: u8,

    /// Show detailed command explanation inline
    #[arg(short = 'd', long = "detail")]
    pub detail: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Fix the last failed command
    Fix,
    /// Interactive chat mode with context
    Chat,
    /// Initialize or show configuration
    Config {
        /// Initialize default config file
        #[arg(long)]
        init: bool,
        /// Show current configuration (API keys masked)
        #[arg(long)]
        show: bool,
        /// Show current configuration with raw secret values
        #[arg(long)]
        raw: bool,
        /// Reset configuration (delete config file)
        #[arg(long)]
        reset: bool,
    },
    /// Clear the command cache
    ClearCache,
    /// View command execution history
    History {
        /// Search pattern to filter history
        search: Option<String>,
        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell)
        shell: clap_complete::Shell,
    },
    /// Generate shell integration code (bash, zsh, fish, powershell, cmd)
    Init {
        /// Shell type
        shell: String,
    },
    /// Check for updates and upgrade piz
    Update,
}

impl Cli {
    pub fn generate_completions(shell: clap_complete::Shell) {
        clap_complete::generate(shell, &mut Self::command(), "piz", &mut std::io::stdout());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn detail_flag_short() {
        let cli = Cli::try_parse_from(["piz", "-d", "list", "files"]).unwrap();
        assert!(cli.detail);
    }

    #[test]
    fn detail_flag_long() {
        let cli = Cli::try_parse_from(["piz", "--detail", "list", "files"]).unwrap();
        assert!(cli.detail);
    }

    #[test]
    fn detail_flag_default_false() {
        let cli = Cli::try_parse_from(["piz", "list", "files"]).unwrap();
        assert!(!cli.detail);
    }

    #[test]
    fn detail_and_explain_no_conflict() {
        let cli = Cli::try_parse_from(["piz", "-d", "-e", "ls -la"]).unwrap();
        assert!(cli.detail);
        assert_eq!(cli.explain.as_deref(), Some("ls -la"));
    }
}

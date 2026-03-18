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

    // ── subcommands ──

    #[test]
    fn subcommand_fix() {
        let cli = Cli::try_parse_from(["piz", "fix"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Fix)));
    }

    #[test]
    fn subcommand_chat() {
        let cli = Cli::try_parse_from(["piz", "chat"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Chat)));
    }

    #[test]
    fn subcommand_config_show() {
        let cli = Cli::try_parse_from(["piz", "config", "--show"]).unwrap();
        if let Some(Commands::Config { show, .. }) = cli.command {
            assert!(show);
        } else {
            panic!("expected Config subcommand");
        }
    }

    #[test]
    fn subcommand_config_raw() {
        let cli = Cli::try_parse_from(["piz", "config", "--raw"]).unwrap();
        if let Some(Commands::Config { raw, .. }) = cli.command {
            assert!(raw);
        } else {
            panic!("expected Config subcommand");
        }
    }

    #[test]
    fn subcommand_config_reset() {
        let cli = Cli::try_parse_from(["piz", "config", "--reset"]).unwrap();
        if let Some(Commands::Config { reset, .. }) = cli.command {
            assert!(reset);
        } else {
            panic!("expected Config subcommand");
        }
    }

    #[test]
    fn subcommand_config_init() {
        let cli = Cli::try_parse_from(["piz", "config", "--init"]).unwrap();
        if let Some(Commands::Config { init, .. }) = cli.command {
            assert!(init);
        } else {
            panic!("expected Config subcommand");
        }
    }

    #[test]
    fn subcommand_clear_cache() {
        let cli = Cli::try_parse_from(["piz", "clear-cache"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::ClearCache)));
    }

    #[test]
    fn subcommand_history_default() {
        let cli = Cli::try_parse_from(["piz", "history"]).unwrap();
        if let Some(Commands::History { search, limit }) = cli.command {
            assert!(search.is_none());
            assert_eq!(limit, 20);
        } else {
            panic!("expected History subcommand");
        }
    }

    #[test]
    fn subcommand_history_with_search() {
        let cli = Cli::try_parse_from(["piz", "history", "git"]).unwrap();
        if let Some(Commands::History { search, .. }) = cli.command {
            assert_eq!(search.as_deref(), Some("git"));
        } else {
            panic!("expected History subcommand");
        }
    }

    #[test]
    fn subcommand_history_with_limit() {
        let cli = Cli::try_parse_from(["piz", "history", "-l", "50"]).unwrap();
        if let Some(Commands::History { limit, .. }) = cli.command {
            assert_eq!(limit, 50);
        } else {
            panic!("expected History subcommand");
        }
    }

    #[test]
    fn subcommand_init() {
        let cli = Cli::try_parse_from(["piz", "init", "bash"]).unwrap();
        if let Some(Commands::Init { shell }) = cli.command {
            assert_eq!(shell, "bash");
        } else {
            panic!("expected Init subcommand");
        }
    }

    #[test]
    fn subcommand_update() {
        let cli = Cli::try_parse_from(["piz", "update"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Update)));
    }

    // ── flags ──

    #[test]
    fn flag_pipe_mode() {
        let cli = Cli::try_parse_from(["piz", "--pipe", "list", "files"]).unwrap();
        assert!(cli.pipe);
    }

    #[test]
    fn flag_eval_mode() {
        let cli = Cli::try_parse_from(["piz", "--eval", "list", "files"]).unwrap();
        assert!(cli.eval);
    }

    #[test]
    fn flag_no_cache() {
        let cli = Cli::try_parse_from(["piz", "--no-cache", "list", "files"]).unwrap();
        assert!(cli.no_cache);
    }

    #[test]
    fn flag_verbose() {
        let cli = Cli::try_parse_from(["piz", "--verbose", "list", "files"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn flag_backend_override() {
        let cli = Cli::try_parse_from(["piz", "-b", "claude", "list", "files"]).unwrap();
        assert_eq!(cli.backend.as_deref(), Some("claude"));
    }

    #[test]
    fn flag_candidates() {
        let cli = Cli::try_parse_from(["piz", "-n", "3", "list", "files"]).unwrap();
        assert_eq!(cli.candidates, 3);
    }

    #[test]
    fn candidates_default_is_1() {
        let cli = Cli::try_parse_from(["piz", "list", "files"]).unwrap();
        assert_eq!(cli.candidates, 1);
    }

    #[test]
    fn flag_explain() {
        let cli = Cli::try_parse_from(["piz", "-e", "ls -la"]).unwrap();
        assert_eq!(cli.explain.as_deref(), Some("ls -la"));
    }

    #[test]
    fn query_captures_multiple_words() {
        let cli = Cli::try_parse_from(["piz", "list", "all", "files"]).unwrap();
        assert_eq!(cli.query, vec!["list", "all", "files"]);
    }

    #[test]
    fn no_args_no_panic() {
        // piz with no args should parse OK (query is empty vec)
        let cli = Cli::try_parse_from(["piz"]).unwrap();
        assert!(cli.query.is_empty());
        assert!(cli.command.is_none());
    }
}

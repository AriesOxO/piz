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
}

impl Cli {
    pub fn generate_completions(shell: clap_complete::Shell) {
        clap_complete::generate(
            shell,
            &mut Self::command(),
            "piz",
            &mut std::io::stdout(),
        );
    }
}

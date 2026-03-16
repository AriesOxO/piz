use clap::{Parser, Subcommand};

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

    /// LLM backend to use (openai, claude, ollama)
    #[arg(short, long)]
    pub backend: Option<String>,

    /// Skip cache lookup
    #[arg(long)]
    pub no_cache: bool,

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
    },
    /// Clear the command cache
    ClearCache,
}

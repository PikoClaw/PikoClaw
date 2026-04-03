use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "pikoclaw", about = "High-performance AI agent for developers")]
#[command(version)]
pub struct Cli {
    #[arg(short = 'p', long, value_name = "PROMPT")]
    pub print: Option<String>,

    #[arg(short = 'd', long, default_value = "false")]
    pub debug: bool,

    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    #[arg(long, value_name = "TURNS")]
    pub max_turns: Option<usize>,

    #[arg(long, value_name = "PROMPT")]
    pub system_prompt: Option<String>,

    #[arg(long, default_value = "false")]
    pub dangerously_skip_permissions: bool,

    #[arg(short = 'n', long, value_name = "NAME")]
    pub name: Option<String>,

    /// Maximum session cost in USD (e.g., 5.00). Session stops when limit is reached.
    #[arg(long, value_name = "USD")]
    pub max_budget_usd: Option<f64>,

    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub output_format: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(short_flag = 'c', about = "Continue the most recent session")]
    Continue,

    #[command(short_flag = 'r', about = "Resume a session by ID")]
    Resume {
        #[arg(value_name = "SESSION_ID")]
        session_id: String,
    },
}

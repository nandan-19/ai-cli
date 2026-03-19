use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ai",
    about = "CLI AI assistant powered by Groq",
    version,
    arg_required_else_help = false
)]
pub struct Cli {
    #[arg(num_args = 0..)]
    pub prompt: Vec<String>,

    #[arg(
        short = 'p',
        long = "prev",
        help = "Send with session history as context"
    )]
    pub prev_context: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(name = "set-key")]
    SetKey { key: String },

    #[command(name = "set-model")]
    SetModel { model: String },

    History,
    Clear,

    #[command(name = "clean-all")]
    CleanAll,

    #[command(name = "rec")]
    Rec {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        cmd_args: Vec<String>,
    },
}

mod agent;
mod cli;
mod commands;
mod config;
mod markdown;
mod session;
mod tools;

use clap::Parser;
use cli::Cli;
use config::load_config;
use session::{clean_orphaned_sessions, load_session};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    clean_orphaned_sessions();

    let cli = Cli::parse();
    let mut config = load_config();

    // Route Subcommands
    if let Some(cmd) = cli.command {
        return commands::route(cmd, &mut config).await;
    }

    let api_key = match &config.api_key {
        Some(k) => k.clone(),
        None => {
            println!("No API key set. Run:  ai set-key YOUR_KEY");
            return Ok(());
        }
    };

    let prompt = cli.prompt.join(" ");
    let mut session = load_session();

    // Delegate to the agent loop
    agent::run_chat_loop(&prompt, &config, &api_key, &mut session).await?;

    Ok(())
}

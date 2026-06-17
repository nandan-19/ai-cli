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

    // Default Agent Flow
    if cli.prompt.is_empty() {
        println!(
            "\n\x1b[1;36mai\x1b[0m  \x1b[2mv{}\x1b[0m — terminal AI assistant powered by Groq\n",
            env!("CARGO_PKG_VERSION")
        );
        println!("\x1b[1mUsage\x1b[0m");
        println!(
            "  ai \x1b[36m<question>\x1b[0m              ask anything — session history is always included"
        );
        println!();
        println!("\x1b[1mSubcommands\x1b[0m");
        println!(
            "  ai \x1b[36mrec\x1b[0m \x1b[2m<cmd>\x1b[0m               run a command and capture output into history"
        );
        println!(
            "  ai \x1b[36mcommit\x1b[0m                  analyze git diff and auto-generate a commit"
        );
        println!(
            "  ai \x1b[36mstream-toggle\x1b[0m           switch between live streaming and markdown mode"
        );
        println!(
            "  ai \x1b[36mhistory\x1b[0m                 print this session's conversation history"
        );
        println!(
            "  ai \x1b[36mclear\x1b[0m                   wipe this session's conversation history"
        );
        println!(
            "  ai \x1b[36mclean-all\x1b[0m               remove orphaned session files from closed terminals"
        );
        println!(
            "  ai \x1b[36mset-key\x1b[0m \x1b[2m<key>\x1b[0m            save your Groq API key"
        );
        println!(
            "  ai \x1b[36mset-model\x1b[0m \x1b[2m<model>\x1b[0m         set the model for all queries"
        );
        println!("\x1b[1mNote\x1b[0m");
        println!(
            "  Prompts with shell-special characters (\x1b[33m' \" ; : &\x1b[0m) must be quoted:"
        );
        println!("  ai \x1b[36m\"what's the difference between TCP and UDP?\"\x1b[0m");
        println!("  ai \x1b[36m\"use ; to separate commands in bash\"\x1b[0m");
        println!();
        println!("\x1b[2mRun `ai <subcommand> --help` for details on any command.\x1b[0m\n");
        return Ok(());
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

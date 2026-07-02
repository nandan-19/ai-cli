pub mod commit;
pub mod history;
pub mod models;
pub mod record;
pub mod update;
use crate::cli::Commands;
use crate::config::Config;

pub async fn route(cmd: Commands, config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Commit => commit::execute_commit(config).await?,
        Commands::SetKey { key } => models::execute_set_key(key, config).await?,
        Commands::SetModel { model } => models::execute_set_model(model, config).await?,
        Commands::History => history::execute_history().await?,
        Commands::Clear => history::execute_clear().await?,
        Commands::CleanAll => history::execute_clean_all().await?,
        Commands::StreamToggle => models::execute_stream_toggle(config).await?,
        Commands::Rec { cmd_args } => record::execute_rec(&cmd_args).await?,
        Commands::Update => update::execute_update().await?, // <-- Add this
    }
    Ok(())
}

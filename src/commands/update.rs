use reqwest::header::USER_AGENT;
use serde_json::Value;
use std::io::Write;
use tempfile::Builder;

pub async fn execute_update() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    println!("\x1b[2mChecking for updates...\x1b[0m");

    // 1. Fetch latest release from GitHub
    let res = client
        .get("https://api.github.com/repos/nandan-19/ai-cli/releases/latest")
        .header(USER_AGENT, "ai-cli-updater")
        .send()
        .await?;

    if !res.status().is_success() {
        eprintln!(
            "\x1b[31mFailed to check for updates.\x1b[0m API responded with: {}",
            res.status()
        );
        return Ok(());
    }

    let release_info: Value = res.json().await?;
    let latest_tag = release_info["tag_name"].as_str().unwrap_or("");
    let latest_version = latest_tag.trim_start_matches('v');
    let current_version = env!("CARGO_PKG_VERSION");

    // 2. Compare versions
    if latest_version == current_version {
        println!(
            "You are already running the latest version (\x1b[36mv{}\x1b[0m).",
            current_version
        );
        return Ok(());
    }

    println!(
        "Update found: \x1b[33mv{}\x1b[0m -> \x1b[32mv{}\x1b[0m",
        current_version, latest_version
    );
    println!("\x1b[2mDownloading...\x1b[0m");

    // 3. Find the correct asset for the OS
    let asset_name = if cfg!(target_os = "windows") {
        "ai.exe"
    } else {
        "ai"
    };
    let assets = release_info["assets"]
        .as_array()
        .ok_or("No assets found in release")?;
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(asset_name))
        .ok_or(format!(
            "Could not find '{}' in the release assets.",
            asset_name
        ))?;

    let download_url = asset["browser_download_url"].as_str().unwrap();

    // 4. Download to a temporary file
    let mut response = client
        .get(download_url)
        .header(USER_AGENT, "ai-cli-updater")
        .send()
        .await?;

    let mut tmp_file = Builder::new().prefix("ai_update").tempfile()?;
    while let Some(chunk) = response.chunk().await? {
        tmp_file.write_all(&chunk)?;
    }

    // Ensure permissions are set for execution (Required for Linux/macOS)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(tmp_file.path())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(tmp_file.path(), perms)?;
    }

    // 5. Swap the current running binary with the downloaded one
    println!("\x1b[2mInstalling...\x1b[0m");
    self_replace::self_replace(tmp_file.path())?;

    println!(
        "✓ Successfully updated to \x1b[32mv{}\x1b[0m!",
        latest_version
    );
    Ok(())
}

const REPO_OWNER: &str = "Varenik-vkusny";
const REPO_NAME: &str = "blackbox";

pub async fn run_update() {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{}", current_version);

    let asset_name = format_asset_name();
    let latest_version = match fetch_latest_version(&asset_name).await {
        Some(v) => v,
        None => {
            eprintln!("Failed to check for updates. This can happen when:");
            eprintln!("  - No release exists yet for your platform");
            eprintln!("  - GitHub is unreachable");
            eprintln!("  - You are behind a proxy/firewall");
            eprintln!();
            eprintln!("You can always download the latest release manually:");
            eprintln!("  https://github.com/{}/{}/releases/latest", REPO_OWNER, REPO_NAME);
            std::process::exit(1);
        }
    };

    if latest_version == current_version {
        println!("Already on latest version (v{}).", current_version);
        return;
    }

    println!("New version available: v{} (current: v{})", latest_version, current_version);

    let download_url = format!(
        "https://github.com/{}/{}/releases/download/v{}/{}",
        REPO_OWNER, REPO_NAME, latest_version, asset_name
    );

    println!("Downloading {}...", asset_name);

    match download_and_replace(&download_url, &asset_name).await {
        Ok(()) => println!("Updated to v{}. Restart BlackBox to use the new version.", latest_version),
        Err(e) => {
            eprintln!("Update failed: {}", e);
            #[cfg(target_os = "windows")]
            eprintln!("On Windows, make sure BlackBox is not running as an MCP server before updating.");
            std::process::exit(1);
        }
    }
}

/// Fetch the latest version by reading the first redirect Location header.
/// GitHub redirects: /releases/latest/download/xxx.zip -> 302 -> /releases/download/vX.Y.Z/xxx.zip
async fn fetch_latest_version(asset_name: &str) -> Option<String> {
    let url = format!(
        "https://github.com/{}/{}/releases/latest/download/{}",
        REPO_OWNER, REPO_NAME, asset_name
    );

    // Disable redirects so we can read the Location header of the first 302.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;

    let resp = client
        .head(&url)
        .header("User-Agent", "blackbox-updater")
        .send()
        .await
        .ok()?;

    // Expect 302 Found with Location header.
    if resp.status() != 302 {
        return None;
    }

    let location = resp.headers().get("location")?.to_str().ok()?;

    // Extract version from URL like .../download/v0.1.2/blackbox-windows-x64.zip
    let version = location
        .split("/download/v")
        .nth(1)?
        .split('/')
        .next()?;

    Some(version.to_string())
}

fn format_asset_name() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    };

    if cfg!(target_os = "windows") {
        format!("blackbox-{}-{}.zip", os, arch)
    } else {
        format!("blackbox-{}-{}.tar.gz", os, arch)
    }
}

async fn download_and_replace(url: &str, asset_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let bytes = client
        .get(url)
        .header("User-Agent", "blackbox-updater")
        .send()
        .await?
        .bytes()
        .await?;

    let current_exe = std::env::current_exe()?;
    let temp_dir = tempfile::tempdir()?;

    if asset_name.ends_with(".zip") {
        let zip_path = temp_dir.path().join("download.zip");
        tokio::fs::write(&zip_path, &bytes).await?;
        extract_zip(&zip_path, temp_dir.path()).await?;
    } else {
        let tar_path = temp_dir.path().join("download.tar.gz");
        tokio::fs::write(&tar_path, &bytes).await?;
        extract_tar(&tar_path, temp_dir.path()).await?;
    }

    let new_binary = if cfg!(target_os = "windows") {
        temp_dir.path().join("blackbox.exe")
    } else {
        temp_dir.path().join("blackbox")
    };

    if !new_binary.exists() {
        return Err("Downloaded archive does not contain 'blackbox' binary".into());
    }

    replace_binary(&new_binary, &current_exe).await?;
    Ok(())
}

async fn extract_zip(zip_path: &std::path::Path, dest: &std::path::Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let zip_path = zip_path.to_path_buf();
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(dest)?;
        Result::<(), Box<dyn std::error::Error + Send + Sync>>::Ok(())
    }).await??;
    Ok(())
}

async fn extract_tar(tar_path: &std::path::Path, dest: &std::path::Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tar_path = tar_path.to_path_buf();
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(tar_path)?;
        let dec = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(dec);
        archive.unpack(dest)?;
        Result::<(), Box<dyn std::error::Error + Send + Sync>>::Ok(())
    }).await??;
    Ok(())
}

async fn replace_binary(new: &std::path::Path, current: &std::path::Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    #[cfg(not(target_os = "windows"))]
    {
        tokio::fs::copy(new, current).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(current).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(current, perms).await?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let old = current.with_extension("exe.old");
        if old.exists() {
            tokio::fs::remove_file(&old).await.ok();
        }

        // On Windows, we cannot delete/replace a running .exe.
        // Strategy: rename current -> .old, then copy new in place.
        tokio::fs::rename(current, &old).await?;
        tokio::fs::copy(new, current).await?;
        println!("Old binary saved as {}. You can delete it after confirming the update.", old.display());
    }

    Ok(())
}

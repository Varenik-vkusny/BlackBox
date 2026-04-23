use serde::Deserialize;

const REPO_OWNER: &str = "Varenik-vkusny";
const REPO_NAME: &str = "blackbox";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub async fn run_update() {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{}", current_version);

    let release = match fetch_latest_release().await {
        Some(r) => r,
        None => {
            eprintln!("Failed to check for updates");
            std::process::exit(1);
        }
    };

    let latest_version = release.tag_name.trim_start_matches('v');
    if latest_version == current_version {
        println!("Already on latest version (v{}).", current_version);
        return;
    }

    println!("New version available: v{}", latest_version);

    let asset_name = format_asset_name();
    let asset = match release.assets.iter().find(|a| a.name == asset_name) {
        Some(a) => a,
        None => {
            eprintln!("No release asset found for this platform: {}", asset_name);
            std::process::exit(1);
        }
    };

    println!("Downloading {}...", asset.name);

    match download_and_replace(&asset.browser_download_url, &asset_name).await {
        Ok(()) => println!("Updated to v{}. Restart BlackBox to use the new version.", latest_version),
        Err(e) => {
            eprintln!("Update failed: {}", e);
            std::process::exit(1);
        }
    }
}

async fn fetch_latest_release() -> Option<Release> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "blackbox-updater")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    resp.json::<Release>().await.ok()
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
    let bytes = client.get(url).send().await?.bytes().await?;

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

    #[cfg(not(target_os = "windows"))]
    let new_binary = temp_dir.path().join("blackbox");
    #[cfg(target_os = "windows")]
    let new_binary = temp_dir.path().join("blackbox.exe");

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
        tokio::fs::rename(current, &old).await?;
        tokio::fs::copy(new, current).await?;
        println!("Old binary saved as {}. Delete it after confirming the update.", old.display());
    }

    Ok(())
}

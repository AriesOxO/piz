use anyhow::{Context, Result};
use colored::*;
use serde::Deserialize;

const GITHUB_API_LATEST: &str = "https://api.github.com/repos/AriesOxO/piz/releases/latest";
const CHECK_INTERVAL_SECS: u64 = 86400; // 24 hours

/// Minimal GitHub release info
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// Stored last-check timestamp
#[derive(Debug, Deserialize, serde::Serialize, Default)]
struct UpdateState {
    last_check: u64,
    latest_version: String,
}

fn state_path() -> Result<std::path::PathBuf> {
    Ok(crate::config::piz_dir()?.join("update_state.json"))
}

fn load_state() -> UpdateState {
    state_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(state: &UpdateState) {
    if let Ok(path) = state_path() {
        let _ = std::fs::write(path, serde_json::to_string(state).unwrap_or_default());
    }
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Parse version string like "v0.2.1" or "0.2.1" into (major, minor, patch)
fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.strip_prefix('v').unwrap_or(v);
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

fn is_newer(remote: &str, local: &str) -> bool {
    match (parse_version(remote), parse_version(local)) {
        (Some(r), Some(l)) => r > l,
        _ => false,
    }
}

/// Background update check — prints a hint if new version is available.
/// Called on normal piz usage, at most once per 24 hours.
pub async fn check_update_hint() {
    let state = load_state();
    let now = now_epoch();

    // Throttle: check at most once per CHECK_INTERVAL_SECS
    if now - state.last_check < CHECK_INTERVAL_SECS {
        // Even if throttled, show hint if we already know about a newer version
        if !state.latest_version.is_empty() && is_newer(&state.latest_version, current_version()) {
            print_update_hint(&state.latest_version);
        }
        return;
    }

    // Try to fetch latest version (with short timeout so it doesn't block)
    match fetch_latest_version_quiet().await {
        Some(ver) => {
            save_state(&UpdateState {
                last_check: now,
                latest_version: ver.clone(),
            });
            if is_newer(&ver, current_version()) {
                print_update_hint(&ver);
            }
        }
        None => {
            // Network error, just update timestamp to avoid hammering
            save_state(&UpdateState {
                last_check: now,
                latest_version: state.latest_version,
            });
        }
    }
}

async fn fetch_latest_version_quiet() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;
    let resp = client
        .get(GITHUB_API_LATEST)
        .header("User-Agent", format!("piz/{}", current_version()))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let release: GitHubRelease = resp.json().await.ok()?;
    let ver = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    Some(ver.to_string())
}

fn print_update_hint(latest: &str) {
    eprintln!(
        "\n{} piz {} {} (current: {}). Run `{}` to upgrade.\n",
        "ℹ".blue(),
        latest.green().bold(),
        "is available".dimmed(),
        current_version().dimmed(),
        "piz update".cyan().bold(),
    );
}

/// Interactive update command: fetch latest release, let user choose upgrade method, execute.
pub async fn run_update(tr: &crate::i18n::T) -> Result<()> {
    println!(
        "{} {}",
        "🔄".cyan(),
        if tr.cached.contains("缓存") {
            "正在检查更新..."
        } else {
            "Checking for updates..."
        }
    );

    let release = fetch_latest_release().await?;
    let remote_ver = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);

    let local_ver = current_version();

    if !is_newer(remote_ver, local_ver) {
        println!(
            "{} {} {}",
            "✔".green().bold(),
            "Already up to date:".green(),
            local_ver
        );
        return Ok(());
    }

    println!(
        "  {} {} → {}",
        "New version:".bold(),
        local_ver.dimmed(),
        remote_ver.green().bold()
    );

    // Find the right asset for this platform
    let asset_url = find_platform_asset(&release.assets)?;
    let asset_name = release
        .assets
        .iter()
        .find(|a| a.browser_download_url == asset_url)
        .map(|a| a.name.as_str())
        .unwrap_or("piz");

    println!("  {} {}", "Download:".bold(), asset_name.dimmed());

    let current_exe =
        std::env::current_exe().context("Cannot determine current executable path")?;
    let install_dir = current_exe
        .parent()
        .context("Cannot determine install directory")?;

    println!(
        "  {} {}",
        "Install directory:".bold(),
        install_dir.display().to_string().dimmed()
    );

    // Let user choose upgrade method
    let is_zh = tr.cached.contains("缓存");
    let items = if is_zh {
        vec![
            "覆盖安装（直接替换当前二进制文件）",
            "卸载后重装（先删除旧版本，再安装新版本）",
            "取消",
        ]
    } else {
        vec![
            "Overwrite install (replace current binary in-place)",
            "Uninstall then reinstall (remove old, then install new)",
            "Cancel",
        ]
    };

    let prompt = if is_zh {
        "选择升级方式"
    } else {
        "Select upgrade method"
    };

    let selection = dialoguer::Select::new()
        .with_prompt(prompt)
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => do_overwrite_install(&asset_url, &current_exe, is_zh).await?,
        1 => do_uninstall_reinstall(&asset_url, &current_exe, is_zh).await?,
        _ => {
            println!("{}", if is_zh { "已取消。" } else { "Cancelled." });
            return Ok(());
        }
    }

    // Clear the update state
    save_state(&UpdateState {
        last_check: now_epoch(),
        latest_version: remote_ver.to_string(),
    });

    println!(
        "\n{} {} {}",
        "✔".green().bold(),
        if is_zh {
            "升级完成！当前版本:"
        } else {
            "Upgrade complete! Version:"
        },
        remote_ver.green().bold()
    );

    Ok(())
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client
        .get(GITHUB_API_LATEST)
        .header("User-Agent", format!("piz/{}", current_version()))
        .send()
        .await
        .context("Failed to reach GitHub API")?;
    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned status {}", resp.status());
    }
    let release: GitHubRelease = resp
        .json()
        .await
        .context("Failed to parse GitHub release")?;
    Ok(release)
}

fn find_platform_asset(assets: &[ReleaseAsset]) -> Result<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Map to expected asset name patterns
    let (os_pattern, arch_pattern, ext) = match (os, arch) {
        ("windows", "x86_64") => ("windows", "x86_64", ".zip"),
        ("windows", "aarch64") => ("windows", "aarch64", ".zip"),
        ("macos", "x86_64") => ("macos", "x86_64", ".tar.gz"),
        ("macos", "aarch64") => ("macos", "aarch64", ".tar.gz"),
        ("linux", "x86_64") => ("linux", "x86_64", ".tar.gz"),
        ("linux", "aarch64") => ("linux", "aarch64", ".tar.gz"),
        _ => anyhow::bail!("Unsupported platform: {}-{}", os, arch),
    };

    // Try to find matching asset
    for asset in assets {
        let name = asset.name.to_lowercase();
        if name.contains(os_pattern) && name.contains(arch_pattern) && name.ends_with(ext) {
            return Ok(asset.browser_download_url.clone());
        }
    }

    // Fallback: try less strict matching
    for asset in assets {
        let name = asset.name.to_lowercase();
        if name.contains(os_pattern) && name.ends_with(ext) {
            return Ok(asset.browser_download_url.clone());
        }
    }

    anyhow::bail!(
        "No matching release asset found for {}-{}. Available: {}",
        os,
        arch,
        assets
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

async fn download_to_temp(url: &str, is_zh: bool) -> Result<std::path::PathBuf> {
    let msg = if is_zh {
        "下载中..."
    } else {
        "Downloading..."
    };
    let spinner = crate::ui::create_spinner(msg);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let resp = client
        .get(url)
        .header("User-Agent", format!("piz/{}", current_version()))
        .send()
        .await
        .context("Download failed")?;

    if !resp.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!("Download returned status {}", resp.status());
    }

    let bytes = resp.bytes().await.context("Failed to read download")?;
    spinner.finish_and_clear();

    let tmp_dir = std::env::temp_dir().join("piz-update");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let tmp_file = tmp_dir.join(if url.ends_with(".zip") {
        "piz-update.zip"
    } else {
        "piz-update.tar.gz"
    });
    std::fs::write(&tmp_file, &bytes).context("Failed to write temp file")?;

    println!(
        "  {} {:.1} MB",
        if is_zh { "已下载:" } else { "Downloaded:" },
        bytes.len() as f64 / 1_048_576.0
    );

    Ok(tmp_file)
}

fn extract_binary(archive_path: &std::path::Path) -> Result<std::path::PathBuf> {
    let tmp_dir = archive_path.parent().unwrap().join("extracted");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)?;

    let archive_str = archive_path.to_string_lossy();

    if archive_str.ends_with(".zip") {
        // Use system command for zip extraction
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive_path.display(),
                    tmp_dir.display()
                ),
            ])
            .status()
            .context("Failed to run PowerShell for zip extraction")?;
        if !status.success() {
            anyhow::bail!("Zip extraction failed");
        }
    } else {
        // tar.gz
        let status = std::process::Command::new("tar")
            .args([
                "xzf",
                &archive_path.to_string_lossy(),
                "-C",
                &tmp_dir.to_string_lossy(),
            ])
            .status()
            .context("Failed to run tar for extraction")?;
        if !status.success() {
            anyhow::bail!("Tar extraction failed");
        }
    }

    // Find the piz binary in extracted files
    let binary_name = if cfg!(target_os = "windows") {
        "piz.exe"
    } else {
        "piz"
    };

    // Search recursively
    find_file_recursive(&tmp_dir, binary_name)
        .context(format!("Binary '{}' not found in archive", binary_name))
}

fn find_file_recursive(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.file_name().is_some_and(|n| n == name) {
                return Some(path);
            }
            if path.is_dir() {
                if let Some(found) = find_file_recursive(&path, name) {
                    return Some(found);
                }
            }
        }
    }
    None
}

async fn do_overwrite_install(url: &str, current_exe: &std::path::Path, is_zh: bool) -> Result<()> {
    let archive = download_to_temp(url, is_zh).await?;
    let new_binary = extract_binary(&archive)?;

    println!(
        "  {}",
        if is_zh {
            "正在覆盖安装..."
        } else {
            "Overwriting binary..."
        }
    );

    replace_binary(&new_binary, current_exe)?;
    cleanup_temp(&archive);
    Ok(())
}

async fn do_uninstall_reinstall(
    url: &str,
    current_exe: &std::path::Path,
    is_zh: bool,
) -> Result<()> {
    let archive = download_to_temp(url, is_zh).await?;
    let new_binary = extract_binary(&archive)?;

    println!(
        "  {}",
        if is_zh {
            "正在卸载旧版本..."
        } else {
            "Removing old version..."
        }
    );

    // On Windows, we can't delete a running exe, so we rename it first
    let backup = current_exe.with_extension("old");
    if backup.exists() {
        let _ = std::fs::remove_file(&backup);
    }

    // Rename current → .old
    std::fs::rename(current_exe, &backup)
        .context("Failed to rename current binary (try running as administrator)")?;

    println!(
        "  {}",
        if is_zh {
            "正在安装新版本..."
        } else {
            "Installing new version..."
        }
    );

    // Copy new binary to original location
    if let Err(e) = std::fs::copy(&new_binary, current_exe) {
        // Rollback: restore the old binary
        let _ = std::fs::rename(&backup, current_exe);
        return Err(e).context("Failed to install new binary");
    }

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(current_exe, std::fs::Permissions::from_mode(0o755));
    }

    // Clean up old binary
    let _ = std::fs::remove_file(&backup);
    cleanup_temp(&archive);
    Ok(())
}

fn replace_binary(new_binary: &std::path::Path, target: &std::path::Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        // On Windows, rename running exe to .old, then copy new one
        let backup = target.with_extension("old");
        if backup.exists() {
            let _ = std::fs::remove_file(&backup);
        }
        std::fs::rename(target, &backup)
            .context("Failed to rename current binary (try running as administrator)")?;

        if let Err(e) = std::fs::copy(new_binary, target) {
            // Rollback
            let _ = std::fs::rename(&backup, target);
            return Err(e).context("Failed to copy new binary");
        }
        let _ = std::fs::remove_file(&backup);
    } else {
        std::fs::copy(new_binary, target).context("Failed to replace binary")?;
        // Set executable permission
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755));
        }
    }
    Ok(())
}

fn cleanup_temp(archive: &std::path::Path) {
    if let Some(parent) = archive.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_valid() {
        assert_eq!(parse_version("0.2.1"), Some((0, 2, 1)));
        assert_eq!(parse_version("v1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("v10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn parse_version_invalid() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("1.0"), None);
        assert_eq!(parse_version("abc"), None);
    }

    #[test]
    fn is_newer_works() {
        assert!(is_newer("0.3.0", "0.2.1"));
        assert!(is_newer("0.2.2", "0.2.1"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.2.1", "0.2.1")); // same
        assert!(!is_newer("0.2.0", "0.2.1")); // older
        assert!(!is_newer("0.1.9", "0.2.0")); // older
    }

    #[test]
    fn is_newer_with_v_prefix() {
        assert!(is_newer("v0.3.0", "0.2.1"));
        assert!(is_newer("0.3.0", "v0.2.1"));
    }

    #[test]
    fn current_version_not_empty() {
        assert!(!current_version().is_empty());
    }
}

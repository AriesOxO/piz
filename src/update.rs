use anyhow::{Context, Result};
use colored::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const GITHUB_API_LATEST: &str = "https://api.github.com/repos/AriesOxO/piz/releases/latest";
const CHECK_INTERVAL_SECS: u64 = 86400; // 24 hours

/// Minimal GitHub release info
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize, Clone)]
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
    let path = match state_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[warn] Failed to resolve update state path: {}", e);
            return UpdateState::default();
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => UpdateState::default(), // File may not exist yet, this is expected
    }
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
    let client = build_proxy_client(5).ok()?;
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

    // Find the right asset for this platform and its checksum metadata
    let asset = find_platform_asset(&release.assets)?;
    let checksum_asset = find_checksum_asset(&release.assets)?;

    println!("  {} {}", "Download:".bold(), asset.name.dimmed());
    println!("  {} {}", "Checksum:".bold(), checksum_asset.name.dimmed());

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
        0 => do_overwrite_install(&asset, &checksum_asset, &current_exe, is_zh).await?,
        1 => do_uninstall_reinstall(&asset, &checksum_asset, &current_exe, is_zh).await?,
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
    let client = build_proxy_client(30)?;
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

fn find_platform_asset(assets: &[ReleaseAsset]) -> Result<ReleaseAsset> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Map to expected asset name patterns
    let (os_patterns, arch_pattern, ext) = match (os, arch) {
        ("windows", "x86_64") => (vec!["windows"], "x86_64", ".zip"),
        ("windows", "aarch64") => (vec!["windows"], "aarch64", ".zip"),
        ("macos", "x86_64") => (vec!["apple-darwin", "macos"], "x86_64", ".tar.gz"),
        ("macos", "aarch64") => (vec!["apple-darwin", "macos"], "aarch64", ".tar.gz"),
        ("linux", "x86_64") => (vec!["linux"], "x86_64", ".tar.gz"),
        ("linux", "aarch64") => (vec!["linux"], "aarch64", ".tar.gz"),
        _ => anyhow::bail!("Unsupported platform: {}-{}", os, arch),
    };

    // Try to find matching asset (strict: os + arch + ext)
    for asset in assets {
        let name = asset.name.to_lowercase();
        if os_patterns.iter().any(|p| name.contains(p))
            && name.contains(arch_pattern)
            && name.ends_with(ext)
        {
            return Ok(asset.clone());
        }
    }

    // Fallback: try less strict matching (os + ext only)
    for asset in assets {
        let name = asset.name.to_lowercase();
        if os_patterns.iter().any(|p| name.contains(p)) && name.ends_with(ext) {
            return Ok(asset.clone());
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

fn find_checksum_asset(assets: &[ReleaseAsset]) -> Result<ReleaseAsset> {
    for asset in assets {
        let name = asset.name.to_ascii_lowercase();
        if name == "checksums.txt"
            || name == "sha256sums.txt"
            || name == "sha256sum.txt"
            || name.contains("sha256")
            || name.contains("checksum")
        {
            return Ok(asset.clone());
        }
    }

    anyhow::bail!(
        "No checksum asset found in release. Refusing to update without integrity verification."
    )
}

/// Build a reqwest client that respects https_proxy/HTTPS_PROXY/ALL_PROXY env vars.
fn build_proxy_client(timeout_secs: u64) -> Result<reqwest::Client> {
    let mut builder =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(timeout_secs));

    // reqwest with rustls-tls doesn't auto-detect system proxy,
    // so we manually check common proxy env vars.
    let proxy_url = std::env::var("https_proxy")
        .or_else(|_| std::env::var("HTTPS_PROXY"))
        .or_else(|_| std::env::var("ALL_PROXY"))
        .or_else(|_| std::env::var("all_proxy"));

    if let Ok(proxy) = proxy_url {
        if !proxy.is_empty() {
            builder = builder.proxy(reqwest::Proxy::all(&proxy)?);
        }
    }

    builder.build().context("Failed to build HTTP client")
}

/// Try GitHub mirror URLs for users in regions where github.com downloads are blocked.
/// Checks GITHUB_MIRROR env var first, then falls back to the original URL.
fn get_download_urls(url: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // User-specified mirror takes priority
    if let Ok(mirror) = std::env::var("GITHUB_MIRROR") {
        let mirrored = url.replace("https://github.com", mirror.trim_end_matches('/'));
        urls.push(mirrored);
    }

    // Original URL as fallback
    urls.push(url.to_string());
    urls
}

async fn download_to_temp(url: &str, is_zh: bool) -> Result<std::path::PathBuf> {
    let msg = if is_zh {
        "下载中..."
    } else {
        "Downloading..."
    };
    let spinner = crate::ui::create_spinner(msg);

    let client = build_proxy_client(300)?;
    let download_urls = get_download_urls(url);

    let mut last_err = None;
    let mut resp = None;
    for download_url in &download_urls {
        match client
            .get(download_url)
            .header("User-Agent", format!("piz/{}", current_version()))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                resp = Some(r);
                break;
            }
            Ok(r) => {
                last_err = Some(anyhow::anyhow!("HTTP {}", r.status()));
            }
            Err(e) => {
                last_err = Some(e.into());
            }
        }
    }

    let resp = match resp {
        Some(r) => r,
        None => {
            spinner.finish_and_clear();
            let hint = if is_zh {
                "\n提示: 可设置代理或镜像加速下载:\n  export https_proxy=http://proxy:port\n  export GITHUB_MIRROR=https://mirror.ghproxy.com"
            } else {
                "\nHint: set proxy or mirror for download:\n  export https_proxy=http://proxy:port\n  export GITHUB_MIRROR=https://mirror.ghproxy.com"
            };
            return Err(last_err
                .unwrap_or_else(|| anyhow::anyhow!("Download failed"))
                .context(format!("Download failed{}", hint)));
        }
    };

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

async fn download_text(url: &str) -> Result<String> {
    let client = build_proxy_client(60)?;
    let download_urls = get_download_urls(url);
    let mut last_err = None;

    for download_url in &download_urls {
        match client
            .get(download_url)
            .header("User-Agent", format!("piz/{}", current_version()))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                return resp
                    .text()
                    .await
                    .context("Failed to read checksum response");
            }
            Ok(resp) => last_err = Some(anyhow::anyhow!("HTTP {}", resp.status())),
            Err(e) => last_err = Some(e.into()),
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Checksum download failed")))
}

fn parse_checksums(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let hash = match parts.next() {
            Some(v) if v.len() == 64 && v.chars().all(|c| c.is_ascii_hexdigit()) => {
                v.to_ascii_lowercase()
            }
            _ => continue,
        };
        let file = match parts.next() {
            Some(v) => v.trim_start_matches('*').to_string(),
            None => continue,
        };

        map.insert(file, hash);
    }

    map
}

fn compute_sha256(path: &std::path::Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read downloaded file: {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

async fn download_verified_asset(
    asset: &ReleaseAsset,
    checksum_asset: &ReleaseAsset,
    is_zh: bool,
) -> Result<std::path::PathBuf> {
    let archive = download_to_temp(&asset.browser_download_url, is_zh).await?;
    let checksum_text = download_text(&checksum_asset.browser_download_url).await?;
    let checksums = parse_checksums(&checksum_text);
    let expected = checksums.get(&asset.name).ok_or_else(|| {
        anyhow::anyhow!(
            "Checksum file does not contain an entry for asset '{}'",
            asset.name
        )
    })?;
    let actual = compute_sha256(&archive)?;

    if &actual != expected {
        cleanup_temp(&archive);
        anyhow::bail!(
            "Checksum mismatch for '{}'. Expected {}, got {}",
            asset.name,
            expected,
            actual
        );
    }

    println!(
        "  {} {}",
        if is_zh {
            "校验通过:"
        } else {
            "Checksum verified:"
        },
        asset.name.dimmed()
    );

    Ok(archive)
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

async fn do_overwrite_install(
    asset: &ReleaseAsset,
    checksum_asset: &ReleaseAsset,
    current_exe: &std::path::Path,
    is_zh: bool,
) -> Result<()> {
    let archive = download_verified_asset(asset, checksum_asset, is_zh).await?;
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
    asset: &ReleaseAsset,
    checksum_asset: &ReleaseAsset,
    current_exe: &std::path::Path,
    is_zh: bool,
) -> Result<()> {
    let archive = download_verified_asset(asset, checksum_asset, is_zh).await?;
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

    #[test]
    fn find_checksum_asset_picks_checksums_file() {
        let assets = vec![
            ReleaseAsset {
                name: "piz-windows-x86_64.zip".into(),
                browser_download_url: "https://example.com/piz.zip".into(),
            },
            ReleaseAsset {
                name: "checksums.txt".into(),
                browser_download_url: "https://example.com/checksums.txt".into(),
            },
        ];
        let checksum = find_checksum_asset(&assets).unwrap();
        assert_eq!(checksum.name, "checksums.txt");
    }

    #[test]
    fn parse_checksums_supports_common_formats() {
        let text = "\
7f83b1657ff1fc53b92dc18148a1d65dfa135014aafa6b87f15d7b0f00a08d8d  piz-linux-x86_64.tar.gz\n\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa *piz-windows-x86_64.zip\n";
        let parsed = parse_checksums(text);
        assert_eq!(
            parsed.get("piz-linux-x86_64.tar.gz").map(String::as_str),
            Some("7f83b1657ff1fc53b92dc18148a1d65dfa135014aafa6b87f15d7b0f00a08d8d")
        );
        assert_eq!(
            parsed.get("piz-windows-x86_64.zip").map(String::as_str),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn compute_sha256_matches_known_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.bin");
        std::fs::write(&path, b"abc").unwrap();
        let hash = compute_sha256(&path).unwrap();
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parse_checksums_ignores_invalid_lines() {
        let parsed = parse_checksums(
            "not-a-hash file.tar.gz\n# comment\n123 short.txt\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb valid.txt",
        );
        assert_eq!(parsed.len(), 1);
        assert!(parsed.contains_key("valid.txt"));
    }
}

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Clone)]
pub struct SystemContext {
    pub os: String,
    pub shell: String,
    pub cwd: String,
    pub arch: String,
    pub is_git_repo: bool,
    pub repo_root: Option<String>,
    pub project_root: Option<String>,
    pub package_manager: Option<String>,
    pub package_manager_source: Option<String>,
}

pub fn collect_context() -> SystemContext {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    collect_context_from(&cwd)
}

fn collect_context_from(cwd: &Path) -> SystemContext {
    let os = if cfg!(target_os = "windows") {
        detect_windows_version()
    } else if cfg!(target_os = "macos") {
        "macOS".into()
    } else {
        "Linux".into()
    };

    let shell = detect_shell();
    let arch = std::env::consts::ARCH.to_string();
    let workspace = detect_workspace(cwd);

    SystemContext {
        os,
        shell,
        cwd: cwd.display().to_string(),
        arch,
        is_git_repo: workspace.repo_root.is_some(),
        repo_root: workspace.repo_root.map(|p| p.display().to_string()),
        project_root: workspace.project_root.map(|p| p.display().to_string()),
        package_manager: workspace.package_manager,
        package_manager_source: workspace.package_manager_source,
    }
}

#[derive(Debug, Clone, Default)]
struct WorkspaceContext {
    repo_root: Option<PathBuf>,
    project_root: Option<PathBuf>,
    package_manager: Option<String>,
    package_manager_source: Option<String>,
}

fn detect_workspace(start: &Path) -> WorkspaceContext {
    let mut current = Some(start);
    let mut depth = 0usize;
    let mut result = WorkspaceContext::default();

    while let Some(dir) = current {
        if result.repo_root.is_none() && dir.join(".git").exists() {
            result.repo_root = Some(dir.to_path_buf());
        }

        if result.package_manager.is_none() {
            if let Some((pm, source)) = detect_package_manager_in_dir(dir) {
                result.project_root = Some(dir.to_path_buf());
                result.package_manager = Some(pm);
                result.package_manager_source = Some(source);
            }
        }

        if result.repo_root.is_some() && result.package_manager.is_some() {
            break;
        }

        depth += 1;
        if depth >= 20 {
            break;
        }
        current = dir.parent();
    }

    result
}

fn detect_package_manager_in_dir(dir: &Path) -> Option<(String, String)> {
    let checks: &[(&str, &str)] = &[
        ("Cargo.toml", "cargo"),
        ("package.json", "npm"),
        ("requirements.txt", "pip"),
        ("go.mod", "go"),
        ("pom.xml", "maven"),
        ("build.gradle", "gradle"),
        ("Gemfile", "bundler"),
        ("composer.json", "composer"),
        ("pyproject.toml", "python"),
    ];
    for (file, pm) in checks {
        if dir.join(file).exists() {
            return Some((pm.to_string(), (*file).to_string()));
        }
    }
    None
}

fn detect_windows_version() -> String {
    // Check if running in PowerShell or cmd
    "Windows".into()
}

fn detect_shell() -> String {
    if cfg!(target_os = "windows") {
        // Detect by checking the parent process name
        if let Some(shell) = detect_windows_parent_shell() {
            return shell;
        }
        // Fallback: check SHELL env for Git Bash / MSYS2
        if let Ok(sh) = std::env::var("SHELL") {
            if sh.contains("bash") {
                return "bash".into();
            }
            if sh.contains("zsh") {
                return "zsh".into();
            }
        }
        // Default to cmd on Windows (safer than assuming PowerShell)
        return "cmd".into();
    }

    std::env::var("SHELL")
        .unwrap_or_else(|_| "bash".into())
        .rsplit('/')
        .next()
        .unwrap_or("bash")
        .to_string()
}

/// Detect the parent shell on Windows by walking up the process tree.
/// Returns Some("PowerShell"), Some("cmd"), Some("bash"), etc.
#[cfg(target_os = "windows")]
pub fn detect_windows_parent_shell() -> Option<String> {
    use std::process::Command;

    // Use WMIC to get the parent process ID, then resolve its name.
    // This avoids depending on PSModulePath which exists system-wide.
    let pid = std::process::id();

    // Get parent PID
    let output = Command::new("cmd")
        .args([
            "/C",
            &format!(
                "wmic process where ProcessId={} get ParentProcessId /format:value",
                pid
            ),
        ])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ppid: u32 = stdout.lines().find_map(|line| {
        line.trim()
            .strip_prefix("ParentProcessId=")
            .and_then(|v| v.trim().parse().ok())
    })?;

    // Get parent process name
    let output = Command::new("cmd")
        .args([
            "/C",
            &format!(
                "wmic process where ProcessId={} get Name /format:value",
                ppid
            ),
        ])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Name=")
            .map(|v| v.trim().to_lowercase())
    })?;

    if name.contains("powershell") || name.contains("pwsh") {
        Some("PowerShell".into())
    } else if name.contains("cmd") {
        Some("cmd".into())
    } else if name.contains("bash") {
        Some("bash".into())
    } else if name.contains("zsh") {
        Some("zsh".into())
    } else if name.contains("fish") {
        Some("fish".into())
    } else if name.contains("nu") {
        Some("nu".into())
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub fn detect_windows_parent_shell() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_fields_not_empty() {
        let ctx = collect_context();
        assert!(!ctx.os.is_empty(), "OS should not be empty");
        assert!(!ctx.shell.is_empty(), "shell should not be empty");
        assert!(!ctx.cwd.is_empty(), "cwd should not be empty");
    }

    #[test]
    fn context_arch_not_empty() {
        let ctx = collect_context();
        assert!(!ctx.arch.is_empty(), "arch should not be empty");
    }

    #[test]
    fn context_git_detection() {
        let ctx = collect_context();
        assert!(ctx.is_git_repo, "should detect git repo");
        assert!(ctx.repo_root.is_some(), "should record repo root");
    }

    #[test]
    fn detect_package_manager_finds_cargo() {
        let ctx = collect_context();
        assert_eq!(ctx.package_manager, Some("cargo".to_string()));
        assert_eq!(ctx.package_manager_source, Some("Cargo.toml".to_string()));
    }

    #[test]
    fn context_os_is_known() {
        let ctx = collect_context();
        let valid = ["Windows", "Linux", "macOS"];
        assert!(
            valid.iter().any(|v| ctx.os.contains(v)),
            "OS '{}' should contain one of {:?}",
            ctx.os,
            valid
        );
    }

    #[test]
    fn detect_workspace_finds_git_and_package_manager_from_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let nested = repo.join("apps").join("cli").join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(
            repo.join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n",
        )
        .unwrap();

        let ctx = collect_context_from(&nested);
        assert!(ctx.is_git_repo);
        assert_eq!(ctx.package_manager.as_deref(), Some("cargo"));
        assert_eq!(ctx.package_manager_source.as_deref(), Some("Cargo.toml"));
        assert_eq!(
            ctx.repo_root.as_deref(),
            Some(repo.to_string_lossy().as_ref())
        );
        assert_eq!(
            ctx.project_root.as_deref(),
            Some(repo.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn detect_workspace_distinguishes_repo_root_and_project_root() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let app = repo.join("apps").join("web");
        let nested = app.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(app.join("package.json"), "{}").unwrap();

        let ctx = collect_context_from(&nested);
        assert!(ctx.is_git_repo);
        assert_eq!(
            ctx.repo_root.as_deref(),
            Some(repo.to_string_lossy().as_ref())
        );
        assert_eq!(
            ctx.project_root.as_deref(),
            Some(app.to_string_lossy().as_ref())
        );
        assert_eq!(ctx.package_manager.as_deref(), Some("npm"));
        assert_eq!(ctx.package_manager_source.as_deref(), Some("package.json"));
    }

    #[test]
    fn detect_workspace_returns_none_without_markers() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("plain").join("folder");
        std::fs::create_dir_all(&nested).unwrap();

        let ctx = collect_context_from(&nested);
        assert!(!ctx.is_git_repo);
        assert!(ctx.repo_root.is_none());
        assert!(ctx.project_root.is_none());
        assert!(ctx.package_manager.is_none());
        assert!(ctx.package_manager_source.is_none());
    }
}

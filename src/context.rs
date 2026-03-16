use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SystemContext {
    pub os: String,
    pub shell: String,
    pub cwd: String,
    pub arch: String,
    pub is_git_repo: bool,
    pub package_manager: Option<String>,
}

pub fn collect_context() -> SystemContext {
    let os = if cfg!(target_os = "windows") {
        detect_windows_version()
    } else if cfg!(target_os = "macos") {
        "macOS".into()
    } else {
        "Linux".into()
    };

    let shell = detect_shell();
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into());
    let arch = std::env::consts::ARCH.to_string();
    let is_git_repo = std::path::Path::new(".git").exists();
    let package_manager = detect_package_manager();

    SystemContext {
        os,
        shell,
        cwd,
        arch,
        is_git_repo,
        package_manager,
    }
}

fn detect_package_manager() -> Option<String> {
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
        if std::path::Path::new(file).exists() {
            return Some(pm.to_string());
        }
    }
    None
}

fn detect_windows_version() -> String {
    // Check if running in PowerShell or cmd
    "Windows".into()
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
        // We're in the piz repo, so .git should exist
        assert!(ctx.is_git_repo, "should detect git repo");
    }

    #[test]
    fn detect_package_manager_finds_cargo() {
        // We're in the piz repo with Cargo.toml
        let pm = detect_package_manager();
        assert_eq!(pm, Some("cargo".to_string()));
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
}

fn detect_shell() -> String {
    if cfg!(target_os = "windows") {
        // Check parent process or common env vars
        if std::env::var("PSModulePath").is_ok() {
            return "PowerShell".into();
        }
        if std::env::var("SHELL")
            .ok()
            .map(|s| s.contains("bash"))
            .unwrap_or(false)
        {
            return "bash".into();
        }
        return "cmd".into();
    }

    std::env::var("SHELL")
        .unwrap_or_else(|_| "bash".into())
        .rsplit('/')
        .next()
        .unwrap_or("bash")
        .to_string()
}

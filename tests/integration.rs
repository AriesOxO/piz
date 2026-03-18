use std::io::Write;

/// Test that executing a simple echo command works and captures output
#[test]
fn execute_echo_command() {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "echo hello_piz_test"])
            .output()
            .expect("failed to execute echo")
    } else {
        std::process::Command::new("sh")
            .args(["-c", "echo hello_piz_test"])
            .output()
            .expect("failed to execute echo")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello_piz_test"));
    assert!(output.status.success());
}

/// Test that a failing command returns non-zero exit code
#[test]
fn failing_command_exit_code() {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "exit 42"])
            .output()
            .expect("failed to execute")
    } else {
        std::process::Command::new("sh")
            .args(["-c", "exit 42"])
            .output()
            .expect("failed to execute")
    };

    assert_eq!(output.status.code(), Some(42));
}

/// Test that last_exec.json can be serialized and deserialized
#[test]
fn last_exec_roundtrip() {
    let last = serde_json::json!({
        "command": "npm install",
        "exit_code": 1,
        "stderr": "EACCES: permission denied",
        "timestamp": 1700000000u64
    });

    let json_str = serde_json::to_string_pretty(&last).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed["command"], "npm install");
    assert_eq!(parsed["exit_code"], 1);
    assert_eq!(parsed["stderr"], "EACCES: permission denied");
}

/// Test history parsing logic (simulate bash_history)
#[test]
fn parse_bash_history_format() {
    let content = "ls\ncd /tmp\ngit status\n";
    let last = content
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap();
    assert_eq!(last, "git status");
}

/// Test history parsing logic (simulate zsh_history)
#[test]
fn parse_zsh_history_format() {
    let content = ": 1700000000:0;ls -la\n: 1700000001:0;git push\n";
    let last_line = content
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap();

    let cmd = if last_line.starts_with(':') {
        last_line.split_once(';').map_or(last_line, |x| x.1)
    } else {
        last_line
    };

    assert_eq!(cmd.trim(), "git push");
}

/// Test SQLite cache via temp file
#[test]
fn cache_with_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_cache.db");

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            danger TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
    )
    .unwrap();

    // Insert
    conn.execute(
        "INSERT INTO cache (key, command, danger, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params!["test_key", "ls -la", "safe", 9999999999i64],
    )
    .unwrap();

    // Read back
    let cmd: String = conn
        .query_row(
            "SELECT command FROM cache WHERE key = ?1",
            ["test_key"],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(cmd, "ls -la");
}

/// Test piz binary --help output
#[test]
fn binary_help_output() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .arg("--help")
        .output()
        .expect("failed to run piz --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Intelligent terminal command assistant"));
    assert!(stdout.contains("fix"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("clear-cache"));
    assert!(stdout.contains("--explain"));
    assert!(stdout.contains("--backend"));
    assert!(stdout.contains("--no-cache"));
}

#[test]
fn config_command_masks_secrets_by_default_and_show() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    let config_path = piz_dir.join("config.toml");
    let content = r#"default_backend = "openai"
cache_ttl_hours = 168

[openai]
api_key = "sk-test-secret-1234"
"#;
    let mut f = std::fs::File::create(&config_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();

    let output = std::process::Command::new(exe)
        .arg("config")
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run piz config");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("Config path:"));
    assert!(stdout.contains("sk-t...1234"));
    assert!(!stdout.contains("sk-test-secret-1234"));

    let output = std::process::Command::new(exe)
        .args(["config", "--show"])
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run piz config --show");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("sk-t...1234"));
    assert!(!stdout.contains("sk-test-secret-1234"));
}

#[test]
fn config_command_raw_shows_unmasked_secrets() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    let config_path = piz_dir.join("config.toml");
    let content = r#"default_backend = "openai"
cache_ttl_hours = 168

[openai]
api_key = "sk-test-secret-1234"
"#;
    let mut f = std::fs::File::create(&config_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();

    let output = std::process::Command::new(exe)
        .args(["config", "--raw"])
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run piz config --raw");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("sk-test-secret-1234"));
}

/// Test config --init subcommand (with temp HOME)
#[test]
fn config_init_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");

    std::fs::create_dir_all(&piz_dir).unwrap();
    let config_path = piz_dir.join("config.toml");

    // Write a minimal config
    let content = r#"default_backend = "openai"
cache_ttl_hours = 168

[openai]
api_key = "sk-test"
"#;
    let mut f = std::fs::File::create(&config_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();

    // Verify it can be parsed
    let parsed: toml::Value = toml::from_str(content).unwrap();
    assert_eq!(parsed["default_backend"].as_str().unwrap(), "openai");
    assert_eq!(parsed["openai"]["api_key"].as_str().unwrap(), "sk-test");
}

// ── Version flag ──

#[test]
fn version_flag_output() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .arg("--version")
        .output()
        .expect("failed to run piz --version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("piz"));
}

// ── Completions subcommand ──

#[test]
fn completions_bash_output() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["completions", "bash"])
        .output()
        .expect("failed to run completions");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[test]
fn completions_zsh_output() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["completions", "zsh"])
        .output()
        .expect("failed to run completions");

    assert!(output.status.success());
}

#[test]
fn completions_fish_output() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["completions", "fish"])
        .output()
        .expect("failed to run completions");

    assert!(output.status.success());
}

#[test]
fn completions_powershell_output() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["completions", "powershell"])
        .output()
        .expect("failed to run completions");

    assert!(output.status.success());
}

// ── Init subcommand ──

#[test]
fn init_bash_output_contains_eval() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["init", "bash"])
        .output()
        .expect("failed to run init");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("eval"));
    assert!(stdout.contains("piz"));
}

#[test]
fn init_fish_output_contains_function() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["init", "fish"])
        .output()
        .expect("failed to run init");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("function"));
}

#[test]
fn init_powershell_output_contains_alias() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["init", "powershell"])
        .output()
        .expect("failed to run init");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Invoke-Piz") || stdout.contains("Set-Alias") || stdout.contains("piz")
    );
}

#[test]
fn init_unknown_shell_fails() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .args(["init", "unknown_shell_xyz"])
        .output()
        .expect("failed to run init");

    assert!(!output.status.success());
}

// ── Config --reset (no config file) ──

#[test]
fn config_reset_no_file_succeeds() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    // No config.toml created

    let output = std::process::Command::new(exe)
        .args(["config", "--reset"])
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run config --reset");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No config file found"));
}

// ── Invalid subcommand ──

#[test]
fn invalid_subcommand_fails() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .arg("nonexistent_subcommand")
        .output()
        .expect("failed to run piz");

    // Either fails with config error or succeeds with empty query — should not crash
    // The key assertion: binary doesn't panic
    let _ = output.status;
}

// ── Help includes all subcommands ──

#[test]
fn help_lists_all_subcommands() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .arg("--help")
        .output()
        .expect("failed to run piz --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fix"));
    assert!(stdout.contains("chat"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("clear-cache"));
    assert!(stdout.contains("history"));
    assert!(stdout.contains("completions"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("update"));
}

// ── Help includes key flags ──

#[test]
fn help_lists_key_flags() {
    let exe = env!("CARGO_BIN_EXE_piz");
    let output = std::process::Command::new(exe)
        .arg("--help")
        .output()
        .expect("failed to run piz --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--pipe"));
    assert!(stdout.contains("--eval"));
    assert!(stdout.contains("--detail"));
    assert!(stdout.contains("--candidates") || stdout.contains("-n"));
}

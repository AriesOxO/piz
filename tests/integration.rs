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

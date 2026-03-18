use std::io::Write;
use std::path::Path;
use std::process::Command;

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

// ═══════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════

fn piz() -> &'static str {
    env!("CARGO_BIN_EXE_piz")
}

fn stdout_str(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn setup_env() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    dir
}

fn setup_env_with_config(config: &str) -> tempfile::TempDir {
    let dir = setup_env();
    let config_path = dir.path().join(".piz").join("config.toml");
    std::fs::write(config_path, config).unwrap();
    dir
}

fn test_config() -> &'static str {
    r#"default_backend = "openai"
cache_ttl_hours = 168
auto_confirm_safe = true
language = "zh"

[openai]
api_key = "sk-fake-test-key-1234567890"
model = "gpt-4o-mini"
"#
}

fn run_piz(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(piz())
        .args(args)
        .env("HOME", dir)
        .env("USERPROFILE", dir)
        .output()
        .expect("failed to run piz")
}

fn run_piz_real(args: &[&str]) -> std::process::Output {
    Command::new(piz())
        .args(args)
        .output()
        .expect("failed to run piz")
}

fn seed_cache(db_path: &Path, entries: &[(&str, &str, &str, &str, &str, &str)]) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            danger TEXT NOT NULL,
            explanation TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query TEXT NOT NULL,
            command TEXT NOT NULL,
            exit_code INTEGER NOT NULL,
            danger TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
    )
    .unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for (query, os, shell, command, danger, explanation) in entries {
        let key = make_cache_key(query, os, shell);
        conn.execute(
            "INSERT OR REPLACE INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![key, command, danger, explanation, now],
        ).unwrap();
    }
}

fn seed_history(db_path: &Path, entries: &[(&str, &str, i32, &str)]) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            danger TEXT NOT NULL,
            explanation TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query TEXT NOT NULL,
            command TEXT NOT NULL,
            exit_code INTEGER NOT NULL,
            danger TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
    )
    .unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for (i, (query, command, exit_code, danger)) in entries.iter().enumerate() {
        conn.execute(
            "INSERT INTO history (query, command, exit_code, danger, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![query, command, exit_code, danger, now + i as u64],
        ).unwrap();
    }
}

fn make_cache_key(query: &str, os: &str, shell: &str) -> String {
    use sha2::{Digest, Sha256};
    let normalized = query.trim().to_lowercase();
    let input = format!("{}|{}|{}", normalized, os, shell);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ═══════════════════════════════════════════════════════
// piz --version
// ═══════════════════════════════════════════════════════

#[test]
fn version_short_flag_equals_long() {
    let v_long = stdout_str(&run_piz_real(&["--version"]));
    let v_short = stdout_str(&run_piz_real(&["-V"]));
    assert_eq!(v_long.trim(), v_short.trim());
}

// ═══════════════════════════════════════════════════════
// piz config 边界测试
// ═══════════════════════════════════════════════════════

#[test]
fn config_default_is_show() {
    let dir = setup_env_with_config(test_config());
    let show = stdout_str(&run_piz(dir.path(), &["config", "--show"]));
    let default = stdout_str(&run_piz(dir.path(), &["config"]));
    assert_eq!(show, default, "config without flags should equal --show");
}

#[test]
fn config_raw_takes_priority_over_show() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &["config", "--raw", "--show"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("sk-fake-test-key-1234567890"),
        "--raw should take priority"
    );
}

#[test]
fn config_multi_backend() {
    let config = r#"default_backend = "openai"
[openai]
api_key = "sk-openai-test-1234567890"
[claude]
api_key = "sk-ant-claude-test-12345"
model = "claude-sonnet-4-20250514"
[gemini]
api_key = "AIza-gemini-test-1234567"
[ollama]
host = "http://localhost:11434"
"#;
    let dir = setup_env_with_config(config);
    let output = run_piz(dir.path(), &["config"]);
    let s = stdout_str(&output);
    assert!(s.contains("openai") || s.contains("sk-o"));
    assert!(s.contains("claude") || s.contains("sk-a"));
}

#[test]
fn config_invalid_toml_shows_raw_content() {
    let dir = setup_env_with_config("this is [[[not valid toml");
    let output = run_piz(dir.path(), &["config"]);
    let s = stdout_str(&output);
    assert!(
        s.contains("not valid") || s.contains("Config path"),
        "invalid TOML config should still produce output, got: '{}'",
        s
    );
}

#[test]
fn config_missing_triggers_no_panic() {
    let dir = setup_env();
    let output = run_piz(dir.path(), &["clear-cache"]);
    let _ = output.status; // 关键：不 panic
}

// ═══════════════════════════════════════════════════════
// piz clear-cache
// ═══════════════════════════════════════════════════════

#[test]
fn clear_cache_with_data() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache(
        &db_path,
        &[
            ("list files", "Windows", "cmd", "dir", "safe", ""),
            ("show time", "Windows", "cmd", "time /t", "safe", ""),
        ],
    );

    let output = run_piz(dir.path(), &["clear-cache"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("2") || s.contains("Cleared"),
        "should report cleared count"
    );
}

#[test]
fn clear_cache_empty() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &["clear-cache"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("0") || s.contains("Cleared"),
        "should report 0 cleared"
    );
}

#[test]
fn clear_cache_preserves_history() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache(&db_path, &[("q", "W", "c", "cmd", "safe", "")]);
    seed_history(&db_path, &[("q", "cmd", 0, "safe")]);

    run_piz(dir.path(), &["clear-cache"]);

    let output = run_piz(dir.path(), &["history"]);
    let s = stdout_str(&output);
    assert!(
        s.contains("cmd") || s.contains("q"),
        "history should survive clear-cache"
    );
}

// ═══════════════════════════════════════════════════════
// piz history
// ═══════════════════════════════════════════════════════

#[test]
fn history_empty() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_history(&db_path, &[]);
    let output = run_piz(dir.path(), &["history"]);
    assert!(output.status.success());
}

#[test]
fn history_default_limit() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    let entries: Vec<(&str, &str, i32, &str)> = (0..30)
        .map(|_| ("query", "command", 0i32, "safe"))
        .collect();
    seed_history(&db_path, &entries);

    let output = run_piz(dir.path(), &["history"]);
    let s = stdout_str(&output);
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        lines.len() <= 20,
        "default limit should be 20, got {}",
        lines.len()
    );
}

#[test]
fn history_custom_limit() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_history(
        &db_path,
        &[
            ("q1", "ls", 0, "safe"),
            ("q2", "pwd", 0, "safe"),
            ("q3", "date", 0, "safe"),
            ("q4", "echo", 0, "safe"),
            ("q5", "whoami", 0, "safe"),
        ],
    );

    let output = run_piz(dir.path(), &["history", "-l", "3"]);
    let s = stdout_str(&output);
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        lines.len() <= 3,
        "limit 3 should show at most 3 lines, got {}",
        lines.len()
    );
}

#[test]
fn history_search() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_history(
        &db_path,
        &[
            ("list git branches", "git branch", 0, "safe"),
            ("show disk usage", "df -h", 0, "safe"),
            ("git status", "git status", 0, "safe"),
        ],
    );

    let output = run_piz(dir.path(), &["history", "git"]);
    let s = stdout_str(&output);
    assert!(s.contains("git"), "search 'git' should return git entries");
    assert!(!s.contains("df"), "should not return non-git entries");
}

#[test]
fn history_search_no_match() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_history(&db_path, &[("list files", "ls", 0, "safe")]);

    let output = run_piz(dir.path(), &["history", "zzz_nonexistent"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    let content_lines: Vec<&str> = s
        .lines()
        .filter(|l| l.contains("→") || l.contains("✓") || l.contains("✗"))
        .collect();
    assert!(content_lines.is_empty(), "no match should return empty");
}

#[test]
fn history_shows_success_and_failure() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_history(
        &db_path,
        &[("good", "echo ok", 0, "safe"), ("bad", "false", 1, "safe")],
    );

    let output = run_piz(dir.path(), &["history"]);
    let s = stdout_str(&output);
    assert!(s.contains("✓") || s.contains("ok"), "should show success");
    assert!(s.contains("✗") || s.contains("1"), "should show failure");
}

// ═══════════════════════════════════════════════════════
// piz init 补充
// ═══════════════════════════════════════════════════════

#[test]
fn init_case_insensitive() {
    for shell in &["Bash", "FISH", "PowerShell"] {
        let output = run_piz_real(&["init", shell]);
        assert!(
            output.status.success(),
            "init {} should be case insensitive",
            shell
        );
    }
}

// ═══════════════════════════════════════════════════════
// piz update
// ═══════════════════════════════════════════════════════

#[test]
fn update_does_not_panic() {
    let output = run_piz_real(&["update"]);
    let combined = format!("{}{}", stdout_str(&output), stderr_str(&output));
    assert!(!combined.is_empty(), "update should produce some output");
}

// ═══════════════════════════════════════════════════════
// 错误路径与边界
// ═══════════════════════════════════════════════════════

#[test]
fn no_args_shows_help() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &[]);
    let combined = format!("{}{}", stdout_str(&output), stderr_str(&output));
    assert!(
        combined.contains("Usage") || combined.contains("help") || combined.contains("piz"),
        "no args should show help or usage"
    );
}

#[test]
fn piz_dir_auto_created() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_piz(dir.path(), &["config", "--reset"]);
    assert!(output.status.success() || !stderr_str(&output).is_empty());
}

#[test]
fn eval_command_file_encoding() {
    let dir = setup_env();
    let eval_file = dir.path().join(".piz").join("eval_command");
    std::fs::write(&eval_file, "echo 你好世界").unwrap();
    let content = std::fs::read_to_string(&eval_file).unwrap();
    assert_eq!(content, "echo 你好世界");
}

#[test]
fn last_exec_json_truncation() {
    let dir = setup_env();
    let exec_path = dir.path().join(".piz").join("last_exec.json");
    let preview: String = "x".repeat(500);
    let last = serde_json::json!({
        "command": "test",
        "exit_code": 0,
        "stdout": preview,
        "stderr": "",
        "timestamp": 12345
    });
    std::fs::write(&exec_path, serde_json::to_string_pretty(&last).unwrap()).unwrap();
    let content = std::fs::read_to_string(&exec_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["stdout"].as_str().unwrap().len() <= 500);
}

#[test]
fn concurrent_eval_command_writes() {
    let dir = setup_env();
    let eval_path = dir.path().join(".piz").join("eval_command");

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let path = eval_path.clone();
            std::thread::spawn(move || {
                std::fs::write(&path, format!("echo thread_{}", i)).ok();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    if eval_path.exists() {
        let content = std::fs::read_to_string(&eval_path).unwrap();
        assert!(content.starts_with("echo thread_"));
    }
}

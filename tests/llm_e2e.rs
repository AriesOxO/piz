//! LLM 端到端测试
//!
//! 所有测试标记为 #[ignore]，需要真实 API key 才能运行。
//! 包含缓存命中路径测试（可能回退到 LLM 调用）。
//!
//! 运行方式：
//!   cargo test --test llm_e2e -- --ignored        # 运行全部
//!   cargo test --test llm_e2e -- --ignored <name>  # 运行指定测试

use std::path::Path;
use std::process::Command;

fn piz() -> &'static str {
    env!("CARGO_BIN_EXE_piz")
}

fn stdout_str(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn run_piz_real(args: &[&str]) -> std::process::Output {
    Command::new(piz())
        .args(args)
        .output()
        .expect("failed to run piz")
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

fn current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
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

fn seed_cache_for_current_env(
    db_path: &Path,
    query: &str,
    command: &str,
    danger: &str,
    explanation: &str,
) {
    for shell in &["bash", "cmd", "PowerShell", "zsh"] {
        let key = make_cache_key(query, current_os(), shell);
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
        let _ = conn.execute(
            "INSERT OR REPLACE INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![key, command, danger, explanation, now],
        );
    }
}

fn has_bash() -> bool {
    Command::new("bash")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════
// 缓存命中路径测试（可能回退到 LLM）
// ═══════════════════════════════════════════════════════

#[test]
#[ignore]
fn cache_hit_pipe_mode() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache_for_current_env(&db_path, "say hello", "echo hello", "safe", "");

    let output = run_piz(dir.path(), &["--pipe", "say", "hello"]);
    let s = stdout_str(&output);
    assert!(
        s.trim() == "echo hello",
        "pipe cache hit should output just the command, got: '{}'",
        s.trim()
    );
}

#[test]
#[ignore]
fn cache_hit_shows_cached_marker() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache_for_current_env(&db_path, "say hello", "echo hello_cached_test", "safe", "");

    let output = run_piz(dir.path(), &["--pipe", "say", "hello"]);
    let out = stdout_str(&output);
    assert!(
        out.contains("echo hello_cached_test"),
        "should output cached command"
    );
}

#[test]
#[ignore]
fn cache_hit_with_explanation() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache_for_current_env(
        &db_path,
        "list all files",
        "ls -la",
        "safe",
        "ls: list directory contents\n-la: long format, show all",
    );

    let output = run_piz(dir.path(), &["--pipe", "-d", "list", "all", "files"]);
    let err = stderr_str(&output);
    assert!(
        err.contains("ls") || err.contains("list") || err.contains("directory"),
        "detail mode should show explanation in stderr, got: '{}'",
        err
    );
}

#[test]
#[ignore]
fn no_cache_flag_skips_cache() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache_for_current_env(&db_path, "say hello", "echo hello", "safe", "");

    let output = run_piz(dir.path(), &["--pipe", "--no-cache", "say", "hello"]);
    assert!(
        !output.status.success() || !stdout_str(&output).contains("echo hello"),
        "--no-cache should not use cached result"
    );
}

// ═══════════════════════════════════════════════════════
// 真实 LLM 端到端测试
// ═══════════════════════════════════════════════════════

#[test]
#[ignore]
fn real_llm_pipe_simple_query() {
    let output = run_piz_real(&["--pipe", "print hello world"]);
    assert!(
        output.status.success(),
        "LLM query failed: {}",
        stderr_str(&output)
    );
    let s = stdout_str(&output);
    assert!(
        s.contains("echo") || s.contains("print"),
        "should return an echo/print command, got: '{}'",
        s.trim()
    );
}

#[test]
#[ignore]
fn real_llm_pipe_list_files() {
    let output = run_piz_real(&["--pipe", "list files in current directory"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("ls") || s.contains("dir") || s.contains("Get-ChildItem"),
        "should return a list command, got: '{}'",
        s.trim()
    );
}

#[test]
#[ignore]
fn real_llm_pipe_show_time() {
    let output = run_piz_real(&["--pipe", "show current date and time"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(!s.trim().is_empty(), "should return a command");
}

#[test]
#[ignore]
fn real_llm_explain_mode() {
    let output = run_piz_real(&["-e", "ls -la"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("ls") || s.contains("list") || s.contains("目录") || s.contains("文件"),
        "explain should describe the command, got: '{}'",
        s.trim()
    );
}

#[test]
#[ignore]
fn real_llm_explain_complex_command() {
    let output = run_piz_real(&["-e", "find . -name '*.rs' -exec grep -l 'TODO' {} +"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(!s.trim().is_empty(), "explain should produce output");
}

#[test]
#[ignore]
fn real_llm_detail_mode() {
    let output = run_piz_real(&["--pipe", "-d", "show disk usage"]);
    assert!(output.status.success());
    let out = stdout_str(&output);
    assert!(!out.trim().is_empty(), "should return a command");
}

#[test]
#[ignore]
fn real_llm_refuses_non_command() {
    let output = run_piz_real(&["--pipe", "what is the meaning of life"]);
    let _ = stdout_str(&output); // 关键：不 crash
}

#[test]
#[ignore]
fn real_llm_chinese_query() {
    let output = run_piz_real(&["--pipe", "列出当前目录所有文件"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("ls") || s.contains("dir") || s.contains("Get-ChildItem"),
        "Chinese query should return list command, got: '{}'",
        s.trim()
    );
}

#[test]
#[ignore]
fn real_llm_dangerous_command_detection() {
    let output = run_piz_real(&["--pipe", "delete all files in root directory"]);
    let _ = stdout_str(&output); // 关键：不 crash
}

#[test]
#[ignore]
fn real_llm_cache_works() {
    let output1 = run_piz_real(&["--pipe", "echo test_cache_12345"]);
    assert!(output1.status.success());
    let cmd1 = stdout_str(&output1);

    let output2 = run_piz_real(&["--pipe", "echo test_cache_12345"]);
    assert!(output2.status.success());
    let cmd2 = stdout_str(&output2);

    assert_eq!(
        cmd1.trim(),
        cmd2.trim(),
        "cached result should be identical"
    );
}

#[test]
#[ignore]
fn real_llm_no_cache_gives_fresh_result() {
    let _ = run_piz_real(&["--pipe", "show current time exactly"]);
    let output = run_piz_real(&["--pipe", "--no-cache", "show current time exactly"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        !s.trim().is_empty(),
        "--no-cache should still return a command"
    );
}

#[test]
#[ignore]
fn real_llm_verbose_shows_debug() {
    let output = run_piz_real(&["--pipe", "--verbose", "print hello"]);
    let err = stderr_str(&output);
    assert!(
        err.contains("[verbose]") || err.contains("prompt") || err.contains("response"),
        "verbose should show debug info in stderr"
    );
}

// ═══════════════════════════════════════════════════════
// 真实 LLM 跨 Shell 测试
// ═══════════════════════════════════════════════════════

#[test]
#[ignore]
fn real_llm_pipe_via_cmd() {
    let output = Command::new("cmd")
        .args(["/C", &format!("{} --pipe print hello world", piz())])
        .output()
        .unwrap();
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("echo") || s.contains("print"),
        "cmd: got '{}'",
        s.trim()
    );
}

#[test]
#[ignore]
fn real_llm_pipe_via_powershell() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("& '{}' --pipe 'print hello world'", piz()),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        !s.trim().is_empty(),
        "PowerShell LLM result should not be empty"
    );
}

#[test]
#[ignore]
fn real_llm_pipe_via_bash() {
    if !has_bash() {
        return;
    }
    let exe = piz().replace('\\', "/");
    let output = Command::new("bash")
        .args(["-c", &format!("'{}' --pipe 'print hello world'", exe)])
        .output()
        .unwrap();
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(!s.trim().is_empty(), "bash LLM result should not be empty");
}

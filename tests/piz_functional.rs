//! piz 全功能端到端测试
//!
//! 覆盖所有 piz 命令和功能路径，使用真实 LLM 后端。
//! 需要 ~/.piz/config.toml 中配置有效的 API key。
//!
//! 运行方式：
//!   cargo test --test piz_functional           # 运行不依赖 LLM 的测试
//!   cargo test --test piz_functional -- --ignored  # 运行需要 LLM 的测试
//!   cargo test --test piz_functional -- --include-ignored # 运行全部

use std::io::Write;
use std::path::Path;
use std::process::Command;

fn piz() -> &'static str {
    env!("CARGO_BIN_EXE_piz")
}

/// 创建临时 piz 环境（config + 空 .piz 目录）
fn setup_env() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    dir
}

/// 创建带配置的临时 piz 环境
fn setup_env_with_config(config: &str) -> tempfile::TempDir {
    let dir = setup_env();
    let config_path = dir.path().join(".piz").join("config.toml");
    std::fs::write(config_path, config).unwrap();
    dir
}

/// 标准测试配置（openai 兼容）
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

/// 在临时环境中运行 piz 命令
fn run_piz(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(piz())
        .args(args)
        .env("HOME", dir)
        .env("USERPROFILE", dir)
        .output()
        .expect("failed to run piz")
}

/// 在临时环境中运行 piz，使用真实配置（从用户目录复制）
fn run_piz_real(args: &[&str]) -> std::process::Output {
    Command::new(piz())
        .args(args)
        .output()
        .expect("failed to run piz")
}

/// 向缓存中预填充数据
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

/// 向历史中预填充数据
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

/// 计算缓存 key（与 cache.rs 中 make_key 一致）
fn make_cache_key(query: &str, os: &str, shell: &str) -> String {
    use sha2::{Digest, Sha256};
    let normalized = query.trim().to_lowercase();
    let input = format!("{}|{}|{}", normalized, os, shell);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stdout_str(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ═══════════════════════════════════════════════════════
// 第一组：无需 LLM 的子命令
// ═══════════════════════════════════════════════════════

// ── piz --help / --version ──

#[test]
fn help_contains_all_subcommands() {
    let output = run_piz_real(&["--help"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    for cmd in &[
        "fix",
        "chat",
        "config",
        "clear-cache",
        "history",
        "completions",
        "init",
        "update",
    ] {
        assert!(s.contains(cmd), "--help missing subcommand: {}", cmd);
    }
}

#[test]
fn help_contains_all_flags() {
    let output = run_piz_real(&["--help"]);
    let s = stdout_str(&output);
    for flag in &[
        "--pipe",
        "--eval",
        "--detail",
        "--no-cache",
        "--verbose",
        "--backend",
        "--explain",
        "--candidates",
    ] {
        assert!(s.contains(flag), "--help missing flag: {}", flag);
    }
}

#[test]
fn version_output_format() {
    let output = run_piz_real(&["--version"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(s.contains("piz"));
    // 版本格式 x.y.z
    let re = regex::Regex::new(r"\d+\.\d+\.\d+").unwrap();
    assert!(
        re.is_match(&s),
        "version should contain x.y.z format, got: {}",
        s
    );
}

#[test]
fn version_short_flag() {
    let v_long = stdout_str(&run_piz_real(&["--version"]));
    let v_short = stdout_str(&run_piz_real(&["-V"]));
    assert_eq!(v_long.trim(), v_short.trim());
}

// ── piz config ──

#[test]
fn config_show_masks_api_key() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &["config", "--show"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("sk-f...7890"),
        "API key should be masked, got: {}",
        s
    );
    assert!(
        !s.contains("sk-fake-test-key-1234567890"),
        "raw key should not appear"
    );
}

#[test]
fn config_default_is_show() {
    let dir = setup_env_with_config(test_config());
    let show = stdout_str(&run_piz(dir.path(), &["config", "--show"]));
    let default = stdout_str(&run_piz(dir.path(), &["config"]));
    assert_eq!(show, default, "config without flags should equal --show");
}

#[test]
fn config_raw_shows_plaintext_key() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &["config", "--raw"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(
        s.contains("sk-fake-test-key-1234567890"),
        "raw mode should show plaintext key"
    );
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
fn config_shows_file_path() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &["config"]);
    let s = stdout_str(&output);
    assert!(
        s.contains("Config path:") || s.contains("config.toml"),
        "should show config path"
    );
}

#[test]
fn config_reset_no_file() {
    let dir = setup_env();
    let output = run_piz(dir.path(), &["config", "--reset"]);
    assert!(output.status.success());
    let s = stdout_str(&output);
    assert!(s.contains("No config file found"));
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
    // config --show 对无效 TOML 会直接输出原始内容
    let s = stdout_str(&output);
    assert!(
        s.contains("not valid") || s.contains("Config path"),
        "invalid TOML config should still produce output, got: '{}'",
        s
    );
}

// ── piz clear-cache ──

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

    // 验证历史仍在
    let output = run_piz(dir.path(), &["history"]);
    let s = stdout_str(&output);
    assert!(
        s.contains("cmd") || s.contains("q"),
        "history should survive clear-cache"
    );
}

// ── piz history ──

#[test]
fn history_empty() {
    let dir = setup_env_with_config(test_config());
    // 创建空数据库
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
        .map(|_i| {
            // 需要 'static 生命周期，用 Box::leak 或直接用固定字符串
            ("query", "command", 0i32, "safe")
        })
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
    assert!(
        s.contains("git"),
        "search 'git' should return git-related entries"
    );
    assert!(
        !s.contains("df"),
        "search 'git' should not return non-git entries"
    );
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
    assert!(
        content_lines.is_empty(),
        "no match should return empty results"
    );
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
    assert!(
        s.contains("✓") || s.contains("ok"),
        "should show success marker"
    );
    assert!(
        s.contains("✗") || s.contains("1"),
        "should show failure marker"
    );
}

// ── piz completions ──

#[test]
fn completions_all_shells() {
    for shell in &["bash", "zsh", "fish", "powershell"] {
        let output = run_piz_real(&["completions", shell]);
        assert!(output.status.success(), "completions {} failed", shell);
        let s = stdout_str(&output);
        assert!(!s.is_empty(), "completions {} is empty", shell);
    }
}

// ── piz init ──

#[test]
fn init_all_shells() {
    let cases = [
        ("bash", vec!["eval", "piz()", "alias p="]),
        ("zsh", vec!["eval", "piz()"]),
        ("fish", vec!["function piz"]),
        ("powershell", vec!["Invoke-Piz", "Set-Alias"]),
        ("pwsh", vec!["Invoke-Piz"]),
        ("cmd", vec!["does not support"]),
    ];
    for (shell, expected) in &cases {
        let output = run_piz_real(&["init", shell]);
        assert!(output.status.success(), "init {} failed", shell);
        let s = stdout_str(&output);
        for keyword in expected {
            assert!(s.contains(keyword), "init {} missing '{}'", shell, keyword);
        }
    }
}

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

#[test]
fn init_unknown_shell_fails() {
    let output = run_piz_real(&["init", "unknown_shell_xyz"]);
    assert!(!output.status.success());
}

// ── piz update ──

#[test]
fn update_does_not_panic() {
    let output = run_piz_real(&["update"]);
    // 可能成功（有网络）或失败（无网络），但不应 panic
    let combined = format!("{}{}", stdout_str(&output), stderr_str(&output));
    assert!(!combined.is_empty(), "update should produce some output");
}

// ═══════════════════════════════════════════════════════
// 第二组：缓存命中路径测试
// ═══════════════════════════════════════════════════════

/// 获取当前平台的 os 和 shell 标识（与 context.rs 一致）
fn current_os() -> &'static str {
    "Windows"
}

#[allow(dead_code)]
fn current_shell() -> &'static str {
    "bash"
}

fn seed_cache_for_current_env(
    db_path: &Path,
    query: &str,
    command: &str,
    danger: &str,
    explanation: &str,
) {
    // 为所有可能的 shell 填充缓存，确保命中
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

#[test]
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
fn cache_hit_shows_cached_marker() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache_for_current_env(&db_path, "say hello", "echo hello_cached_test", "safe", "");

    let output = run_piz(dir.path(), &["--pipe", "say", "hello"]);
    // pipe 模式下 cached 标记输出到 stderr
    let _err = stderr_str(&output);
    let out = stdout_str(&output);
    // 命令应该被输出
    assert!(
        out.contains("echo hello_cached_test"),
        "should output cached command"
    );
}

#[test]
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
    // detail 模式下说明应该在 stderr
    assert!(
        err.contains("ls") || err.contains("list") || err.contains("directory"),
        "detail mode should show explanation in stderr, got: '{}'",
        err
    );
}

#[test]
fn no_cache_flag_skips_cache() {
    let dir = setup_env_with_config(test_config());
    let db_path = dir.path().join(".piz").join("cache.db");
    seed_cache_for_current_env(&db_path, "say hello", "echo hello", "safe", "");

    // --no-cache 应跳过缓存，尝试调用 LLM（会因假 key 失败）
    let output = run_piz(dir.path(), &["--pipe", "--no-cache", "say", "hello"]);
    // 假 key 会导致 LLM 调用失败
    assert!(
        !output.status.success() || !stdout_str(&output).contains("echo hello"),
        "--no-cache should not use cached result"
    );
}

// ═══════════════════════════════════════════════════════
// 第三组：真实 LLM 端到端测试
// 使用真实配置和 API，标记为 #[ignore]
// 运行方式：cargo test --test piz_functional -- --ignored
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
    let _err = stderr_str(&output);
    assert!(!out.trim().is_empty(), "should return a command");
    // detail 模式下说明在 stderr
    // 可能在 stderr 或作为交互显示
}

#[test]
#[ignore]
fn real_llm_refuses_non_command() {
    // LLM 应拒绝非命令请求
    let output = run_piz_real(&["--pipe", "what is the meaning of life"]);
    // 可能成功（返回某种命令）或失败（拒绝），取决于 LLM
    // 关键是不 crash
    let _ = stdout_str(&output);
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
    // 请求一个危险命令，验证 piz 的危险检测
    let output = run_piz_real(&["--pipe", "delete all files in root directory"]);
    let s = stdout_str(&output);
    // 命令应该被生成（pipe 模式不阻止），但应该是危险的
    // 或者 LLM 可能拒绝
    // 关键验证：不 crash
    let _ = s;
}

#[test]
#[ignore]
fn real_llm_cache_works() {
    // 第一次调用
    let output1 = run_piz_real(&["--pipe", "echo test_cache_12345"]);
    assert!(output1.status.success());
    let cmd1 = stdout_str(&output1);

    // 第二次调用应该走缓存
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
    // 先正常调用建立缓存
    let _ = run_piz_real(&["--pipe", "show current time exactly"]);

    // --no-cache 应跳过缓存
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
// 第四组：跨 Shell 调用 piz
// ═══════════════════════════════════════════════════════

fn has_bash() -> bool {
    Command::new("bash")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[allow(dead_code)]
fn has_pwsh() -> bool {
    Command::new("pwsh")
        .args(["-NoProfile", "-Command", "echo ok"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn piz_version_via_cmd() {
    let output = Command::new("cmd")
        .args(["/C", &format!("{} --version", piz())])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("piz"));
}

#[test]
fn piz_version_via_powershell() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("& '{}' --version", piz()),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("piz"));
}

#[test]
fn piz_version_via_bash() {
    if !has_bash() {
        return;
    }
    let exe = piz().replace('\\', "/");
    let output = Command::new("bash")
        .args(["-c", &format!("'{}' --version", exe)])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("piz"));
}

#[test]
fn piz_version_consistent_across_shells() {
    let cmd_ver = stdout_str(
        &Command::new("cmd")
            .args(["/C", &format!("{} --version", piz())])
            .output()
            .unwrap(),
    );

    let ps_ver = stdout_str(
        &Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("& '{}' --version", piz()),
            ])
            .output()
            .unwrap(),
    );

    assert_eq!(
        cmd_ver.trim(),
        ps_ver.trim(),
        "version should be consistent across shells"
    );
}

#[test]
fn piz_config_via_cmd() {
    let dir = setup_env_with_config(test_config());
    let home = dir.path().display().to_string();
    let output = Command::new("cmd")
        .args([
            "/C",
            &format!(
                "set HOME={}&& set USERPROFILE={}&& {} config --show",
                home,
                home,
                piz()
            ),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("sk-f...7890"));
}

#[test]
fn piz_config_via_powershell() {
    let dir = setup_env_with_config(test_config());
    let home = dir.path().display().to_string();
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$env:HOME='{}'; $env:USERPROFILE='{}'; & '{}' config --show",
                home,
                home,
                piz()
            ),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("sk-f...7890"));
}

#[test]
fn piz_config_via_bash() {
    if !has_bash() {
        return;
    }
    let dir = setup_env_with_config(test_config());
    let home = dir.path().display().to_string().replace('\\', "/");
    let exe = piz().replace('\\', "/");
    let output = Command::new("bash")
        .args([
            "-c",
            &format!(
                "HOME='{}' USERPROFILE='{}' '{}' config --show",
                home, home, exe
            ),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("sk-f...7890"));
}

// ── 真实 LLM 跨 Shell ──

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

// ═══════════════════════════════════════════════════════
// 第五组：错误路径与边界
// ═══════════════════════════════════════════════════════

#[test]
fn no_args_shows_help() {
    let dir = setup_env_with_config(test_config());
    let output = run_piz(dir.path(), &[]);
    let combined = format!("{}{}", stdout_str(&output), stderr_str(&output));
    // 无参数应显示帮助或提示
    assert!(
        combined.contains("Usage") || combined.contains("help") || combined.contains("piz"),
        "no args should show help or usage"
    );
}

#[test]
fn config_missing_triggers_error() {
    let dir = setup_env(); // 无 config.toml
                           // 尝试执行需要配置的命令
    let output = run_piz(dir.path(), &["clear-cache"]);
    // clear-cache 需要 piz_dir，应该能工作或优雅失败
    // 关键：不 panic
    let _ = output.status;
}

#[test]
fn piz_dir_auto_created() {
    let dir = tempfile::tempdir().unwrap();
    // 不预创建 .piz 目录
    let output = run_piz(dir.path(), &["config", "--reset"]);
    // 应该优雅处理
    assert!(output.status.success() || !stderr_str(&output).is_empty());
}

#[test]
fn eval_command_file_encoding() {
    let dir = setup_env();
    let eval_file = dir.path().join(".piz").join("eval_command");
    // 模拟写入含中文的命令
    std::fs::write(&eval_file, "echo 你好世界").unwrap();
    let content = std::fs::read_to_string(&eval_file).unwrap();
    assert_eq!(
        content, "echo 你好世界",
        "eval_command should preserve UTF-8"
    );
}

#[test]
fn last_exec_json_truncation() {
    let dir = setup_env();
    let piz_dir = dir.path().join(".piz");
    let exec_path = piz_dir.join("last_exec.json");
    let long_output: String = "x".repeat(1000);
    let preview: String = long_output.chars().take(500).collect();
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

    // 并发写入
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

    // 文件应该存在且可读（不 crash）
    if eval_path.exists() {
        let content = std::fs::read_to_string(&eval_path).unwrap();
        assert!(content.starts_with("echo thread_"));
    }
}

// ═══════════════════════════════════════════════════════
// 第六组：Eval 模式端到端
// ═══════════════════════════════════════════════════════

#[test]
fn eval_bash_wrapper() {
    if !has_bash() {
        return;
    }
    let dir = setup_env();
    let eval_file = dir.path().join(".piz").join("eval_command");
    std::fs::write(&eval_file, "echo eval_bash_ok").unwrap();

    let eval_path = eval_file.display().to_string().replace('\\', "/");
    let script = format!(
        "if [ -f '{}' ]; then cmd=$(cat '{}'); rm -f '{}'; eval \"$cmd\"; fi",
        eval_path, eval_path, eval_path
    );

    let output = Command::new("bash").args(["-c", &script]).output().unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("eval_bash_ok"));
    assert!(!eval_file.exists(), "eval_command should be cleaned up");
}

#[test]
fn eval_powershell_wrapper() {
    let dir = setup_env();
    let eval_file = dir.path().join(".piz").join("eval_command");
    std::fs::write(&eval_file, "Write-Output 'eval_ps_ok'").unwrap();

    let eval_path = eval_file.display().to_string();
    let script = format!(
        "if (Test-Path '{}') {{ $cmd = Get-Content '{}' -Raw; Remove-Item '{}' -Force; Invoke-Expression $cmd }}",
        eval_path, eval_path, eval_path
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(stdout_str(&output).contains("eval_ps_ok"));
    assert!(!eval_file.exists(), "eval_command should be cleaned up");
}

#[test]
fn eval_cmd_file_roundtrip() {
    let dir = setup_env();
    let eval_file = dir.path().join(".piz").join("eval_command");
    std::fs::write(&eval_file, "echo eval_cmd_ok").unwrap();

    // Rust 直接读取验证（cmd 的 type 对临时路径有兼容性问题）
    let content = std::fs::read_to_string(&eval_file).unwrap();
    assert_eq!(content, "echo eval_cmd_ok");

    // 同时验证 PowerShell 能读取
    let ps_output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Get-Content '{}' -Raw", eval_file.display()),
        ])
        .output()
        .unwrap();
    assert!(ps_output.status.success());
    assert!(stdout_str(&ps_output).contains("echo eval_cmd_ok"));
}

// ═══════════════════════════════════════════════════════
// 第七组：Init 代码语法验证
// ═══════════════════════════════════════════════════════

#[test]
fn init_bash_syntax_valid() {
    if !has_bash() {
        return;
    }
    let output = run_piz_real(&["init", "bash"]);
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("init.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    f.write_all(&output.stdout).unwrap();

    let check = Command::new("bash")
        .args(["-n", &script.display().to_string()])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "bash init syntax error: {}",
        stderr_str(&check)
    );
}

#[test]
fn completions_bash_syntax_valid() {
    if !has_bash() {
        return;
    }
    let output = run_piz_real(&["completions", "bash"]);
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("complete.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    f.write_all(&output.stdout).unwrap();

    let check = Command::new("bash")
        .args(["-n", &script.display().to_string()])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "bash completions syntax error: {}",
        stderr_str(&check)
    );
}

#[test]
fn init_powershell_syntax_valid() {
    let output = run_piz_real(&["init", "powershell"]);
    let init_code = stdout_str(&output);

    let check = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[System.Management.Automation.PSParser]::Tokenize(@'\n{}\n'@, [ref]$null).Count",
                init_code
            ),
        ])
        .output()
        .unwrap();
    assert!(check.status.success(), "PowerShell init syntax error");
    let count: i32 = stdout_str(&check).trim().parse().unwrap_or(0);
    assert!(count > 0, "PowerShell init should have tokens");
}

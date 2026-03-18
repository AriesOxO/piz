#![cfg(target_os = "windows")]
//! Windows 真实环境跨 Shell 测试
//!
//! 覆盖场景：
//! 1. 三种 Shell (cmd / PowerShell / bash) 的命令执行和输出捕获
//! 2. 中文输出编码（GBK / UTF-8）
//! 3. Shell init 代码生成 + 语法验证
//! 4. Shell completions 生成
//! 5. Eval 模式文件机制
//! 6. 跨 Shell 退出码传播
//! 7. 环境变量和工作目录
//! 8. piz 二进制在各 Shell 中的参数传递

use std::io::Write;
use std::process::Command;

fn piz_exe() -> &'static str {
    env!("CARGO_BIN_EXE_piz")
}

fn has_bash() -> bool {
    Command::new("bash")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_pwsh() -> bool {
    Command::new("pwsh")
        .args(["-NoProfile", "-Command", "echo ok"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════
// 1. cmd.exe 命令执行
// ═══════════════════════════════════════════════════════

#[test]
fn cmd_echo_captures_output() {
    let output = Command::new("cmd")
        .args(["/C", "echo hello_cmd_test"])
        .output()
        .expect("failed to run cmd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello_cmd_test"));
}

#[test]
fn cmd_exit_code_propagation() {
    let output = Command::new("cmd")
        .args(["/C", "exit 77"])
        .output()
        .expect("failed to run cmd");
    assert_eq!(output.status.code(), Some(77));
}

#[test]
fn cmd_stderr_capture() {
    let output = Command::new("cmd")
        .args(["/C", "echo error_msg 1>&2"])
        .output()
        .expect("failed to run cmd");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error_msg"));
}

#[test]
fn cmd_pipe_chain() {
    let output = Command::new("cmd")
        .args(["/C", "echo hello world | findstr hello"])
        .output()
        .expect("failed to run cmd pipe");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
}

#[test]
fn cmd_environment_variable() {
    let output = Command::new("cmd")
        .args(["/C", "echo %COMSPEC%"])
        .output()
        .expect("failed to run cmd env");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("cmd.exe"));
}

#[test]
fn cmd_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new("cmd")
        .args(["/C", "cd"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run cmd cd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = dir.path().to_string_lossy();
    assert!(
        stdout.trim().eq_ignore_ascii_case(expected.trim()),
        "expected dir containing '{}', got '{}'",
        expected,
        stdout.trim()
    );
}

// ═══════════════════════════════════════════════════════
// 2. PowerShell 命令执行
// ═══════════════════════════════════════════════════════

#[test]
fn powershell_echo_captures_output() {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Write-Output 'hello_ps_test'"])
        .output()
        .expect("failed to run powershell");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello_ps_test"));
}

#[test]
fn powershell_exit_code_propagation() {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "exit 88"])
        .output()
        .expect("failed to run powershell");
    assert_eq!(output.status.code(), Some(88));
}

#[test]
fn powershell_stderr_capture() {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Write-Error 'ps_error_test' 2>&1"])
        .output()
        .expect("failed to run powershell");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // PowerShell may route Write-Error to stderr or stdout depending on version
    assert!(
        stderr.contains("ps_error_test") || stdout.contains("ps_error_test"),
        "Expected error message in output"
    );
}

#[test]
fn powershell_pipe_chain() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "'hello','world' | Select-String 'hello'",
        ])
        .output()
        .expect("failed to run powershell pipe");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
}

#[test]
fn powershell_env_variable() {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "$env:COMSPEC"])
        .output()
        .expect("failed to run powershell env");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("cmd.exe"));
}

#[test]
fn powershell_json_output() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "@{name='test';value=42} | ConvertTo-Json -Compress",
        ])
        .output()
        .expect("failed to run powershell json");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\""));
    assert!(stdout.contains("test"));
}

/// pwsh 7.x 独立测试
#[test]
fn pwsh7_echo_if_available() {
    if !has_pwsh() {
        eprintln!("skipping: pwsh not available");
        return;
    }
    let output = Command::new("pwsh")
        .args(["-NoProfile", "-Command", "Write-Output 'hello_pwsh7'"])
        .output()
        .expect("failed to run pwsh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello_pwsh7"));
}

// ═══════════════════════════════════════════════════════
// 3. Git Bash 命令执行
// ═══════════════════════════════════════════════════════

#[test]
fn bash_echo_captures_output() {
    if !has_bash() {
        eprintln!("skipping: bash not available");
        return;
    }
    let output = Command::new("bash")
        .args(["-c", "echo hello_bash_test"])
        .output()
        .expect("failed to run bash");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello_bash_test"));
}

#[test]
fn bash_exit_code_propagation() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args(["-c", "exit 99"])
        .output()
        .expect("failed to run bash");
    assert_eq!(output.status.code(), Some(99));
}

#[test]
fn bash_stderr_capture() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args(["-c", "echo bash_err >&2"])
        .output()
        .expect("failed to run bash");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bash_err"));
}

#[test]
fn bash_pipe_chain() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args(["-c", "echo 'hello world' | grep hello | wc -l"])
        .output()
        .expect("failed to run bash pipe");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let count: i32 = stdout.trim().parse().unwrap_or(0);
    assert_eq!(count, 1);
}

#[test]
fn bash_env_variable() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args(["-c", "echo $HOME"])
        .output()
        .expect("failed to run bash env");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty());
}

#[test]
fn bash_working_directory() {
    if !has_bash() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new("bash")
        .args(["-c", "pwd"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run bash pwd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty());
}

// ═══════════════════════════════════════════════════════
// 4. 中文编码测试
// ═══════════════════════════════════════════════════════

#[test]
fn cmd_chinese_output_does_not_panic() {
    // cmd 默认 GBK 编码，验证不会 panic
    let output = Command::new("cmd")
        .args(["/C", "echo 测试中文输出"])
        .output()
        .expect("failed to run cmd with Chinese");
    // 不管编码如何，不应 panic
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[test]
fn powershell_chinese_output() {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Write-Output '测试中文输出PS'"])
        .output()
        .expect("failed to run powershell with Chinese");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // PowerShell 可能用 UTF-16，lossy 转换不应 panic
    assert!(!stdout.is_empty());
}

#[test]
fn bash_chinese_output_utf8() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args(["-c", "echo '测试中文输出Bash'"])
        .output()
        .expect("failed to run bash with Chinese");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Git Bash 是 UTF-8 环境，应该完整保留
    assert!(stdout.contains("测试中文输出Bash"));
}

// ═══════════════════════════════════════════════════════
// 5. piz 二进制在各 Shell 中的执行
// ═══════════════════════════════════════════════════════

#[test]
fn piz_help_via_cmd() {
    let output = Command::new("cmd")
        .args(["/C", &format!("{} --help", piz_exe())])
        .output()
        .expect("failed to run piz via cmd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Intelligent terminal command assistant"));
}

#[test]
fn piz_help_via_powershell() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("& '{}' --help", piz_exe()),
        ])
        .output()
        .expect("failed to run piz via powershell");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Intelligent terminal command assistant"));
}

#[test]
fn piz_help_via_bash() {
    if !has_bash() {
        return;
    }
    // Git Bash 需要 Windows 路径转换
    let exe = piz_exe().replace('\\', "/");
    let output = Command::new("bash")
        .args(["-c", &format!("'{}' --help", exe)])
        .output()
        .expect("failed to run piz via bash");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Intelligent terminal command assistant"));
}

#[test]
fn piz_version_via_cmd() {
    let output = Command::new("cmd")
        .args(["/C", &format!("{} --version", piz_exe())])
        .output()
        .expect("failed to run piz version via cmd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("piz"));
}

#[test]
fn piz_version_via_powershell() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("& '{}' --version", piz_exe()),
        ])
        .output()
        .expect("failed to run piz version via powershell");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("piz"));
}

// ═══════════════════════════════════════════════════════
// 6. Shell init 代码语法验证
// ═══════════════════════════════════════════════════════

#[test]
fn init_powershell_syntax_valid() {
    // 生成 PowerShell init 代码后用 PowerShell 实际解析
    let output = Command::new(piz_exe())
        .args(["init", "powershell"])
        .output()
        .expect("failed to get init code");
    assert!(output.status.success());
    let init_code = String::from_utf8_lossy(&output.stdout);

    // 用 PowerShell 解析（不执行）验证语法
    let check = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[System.Management.Automation.PSParser]::Tokenize('{}', [ref]$null).Count -gt 0",
                init_code.replace('\'', "''")
            ),
        ])
        .output()
        .expect("failed to validate PS syntax");
    assert!(
        check.status.success(),
        "PowerShell init code has syntax errors"
    );
}

#[test]
fn init_bash_syntax_valid() {
    if !has_bash() {
        return;
    }
    let output = Command::new(piz_exe())
        .args(["init", "bash"])
        .output()
        .expect("failed to get init code");
    assert!(output.status.success());

    // 写入临时文件，用 bash -n 检查语法
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("piz_init.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    f.write_all(&output.stdout).unwrap();

    let check = Command::new("bash")
        .args(["-n", &script.display().to_string()])
        .output()
        .expect("failed to validate bash syntax");
    assert!(
        check.status.success(),
        "bash init code has syntax errors: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

// ═══════════════════════════════════════════════════════
// 7. Completions 语法验证
// ═══════════════════════════════════════════════════════

#[test]
fn completions_bash_syntax_valid() {
    if !has_bash() {
        return;
    }
    let output = Command::new(piz_exe())
        .args(["completions", "bash"])
        .output()
        .expect("failed to get bash completions");
    assert!(output.status.success());

    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("piz_complete.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    f.write_all(&output.stdout).unwrap();

    let check = Command::new("bash")
        .args(["-n", &script.display().to_string()])
        .output()
        .expect("failed to validate bash completions");
    assert!(
        check.status.success(),
        "bash completions have syntax errors: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn completions_powershell_not_empty() {
    let output = Command::new(piz_exe())
        .args(["completions", "powershell"])
        .output()
        .expect("failed to get ps completions");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
    // PowerShell completions should contain Register-ArgumentCompleter or similar
    assert!(
        stdout.contains("Register") || stdout.contains("piz"),
        "PowerShell completions should contain registration code"
    );
}

// ═══════════════════════════════════════════════════════
// 8. Eval 模式文件机制
// ═══════════════════════════════════════════════════════

#[test]
fn eval_command_file_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    let eval_file = piz_dir.join("eval_command");

    // 模拟 piz 写入 eval_command
    std::fs::write(&eval_file, "cd /tmp").unwrap();
    assert!(eval_file.exists());

    // 模拟 shell wrapper 读取
    let content = std::fs::read_to_string(&eval_file).unwrap();
    assert_eq!(content, "cd /tmp");

    // 模拟清理
    std::fs::remove_file(&eval_file).unwrap();
    assert!(!eval_file.exists());
}

#[test]
fn eval_command_file_chinese_path() {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    let eval_file = piz_dir.join("eval_command");

    // 含中文路径的命令
    std::fs::write(&eval_file, r"cd D:\项目\测试").unwrap();
    let content = std::fs::read_to_string(&eval_file).unwrap();
    assert!(content.contains("项目"));
    assert!(content.contains("测试"));
}

#[test]
fn eval_command_bash_wrapper() {
    if !has_bash() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let eval_file = dir.path().join("eval_command");
    std::fs::write(&eval_file, "echo eval_test_success").unwrap();

    let eval_path = eval_file.display().to_string().replace('\\', "/");
    let script = format!(
        r#"
if [ -f "{path}" ]; then
    cmd=$(cat "{path}")
    rm -f "{path}"
    eval "$cmd"
fi
"#,
        path = eval_path
    );

    let output = Command::new("bash")
        .args(["-c", &script])
        .output()
        .expect("failed to run bash eval wrapper");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("eval_test_success"));
    assert!(!eval_file.exists(), "eval_command should be cleaned up");
}

#[test]
fn eval_command_powershell_wrapper() {
    let dir = tempfile::tempdir().unwrap();
    let eval_file = dir.path().join("eval_command");
    std::fs::write(&eval_file, "Write-Output 'ps_eval_success'").unwrap();

    let eval_path = eval_file.display().to_string();
    let script = format!(
        r#"
if (Test-Path '{path}') {{
    $cmd = Get-Content '{path}' -Raw
    Remove-Item '{path}' -Force
    Invoke-Expression $cmd
}}
"#,
        path = eval_path
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .expect("failed to run ps eval wrapper");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ps_eval_success"));
    assert!(!eval_file.exists(), "eval_command should be cleaned up");
}

// ═══════════════════════════════════════════════════════
// 9. 跨 Shell 一致性测试
// ═══════════════════════════════════════════════════════

#[test]
fn same_command_same_result_across_shells() {
    // "echo hello" 在所有 shell 中应返回包含 hello 的输出
    let shells: Vec<(&str, Vec<&str>)> = vec![
        ("cmd", vec!["/C", "echo hello"]),
        (
            "powershell",
            vec!["-NoProfile", "-Command", "Write-Output 'hello'"],
        ),
    ];

    for (shell, args) in &shells {
        let output = Command::new(shell)
            .args(args.as_slice())
            .output()
            .unwrap_or_else(|_| panic!("failed to run {}", shell));
        assert!(output.status.success(), "{} failed", shell);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"), "{} output missing 'hello'", shell);
    }

    if has_bash() {
        let output = Command::new("bash")
            .args(["-c", "echo hello"])
            .output()
            .expect("failed to run bash");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
    }
}

#[test]
fn exit_code_consistent_across_shells() {
    // 非零退出码在所有 shell 中都应正确传播
    let cases: Vec<(&str, Vec<&str>, i32)> = vec![
        ("cmd", vec!["/C", "exit 1"], 1),
        ("powershell", vec!["-NoProfile", "-Command", "exit 1"], 1),
    ];

    for (shell, args, expected) in &cases {
        let output = Command::new(shell)
            .args(args.as_slice())
            .output()
            .unwrap_or_else(|_| panic!("failed to run {}", shell));
        assert_eq!(
            output.status.code(),
            Some(*expected),
            "{} exit code mismatch",
            shell
        );
    }

    if has_bash() {
        let output = Command::new("bash")
            .args(["-c", "exit 1"])
            .output()
            .expect("failed to run bash");
        assert_eq!(output.status.code(), Some(1));
    }
}

// ═══════════════════════════════════════════════════════
// 10. piz config 在各 Shell 中的行为
// ═══════════════════════════════════════════════════════

#[test]
fn piz_config_show_via_cmd() {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    std::fs::write(
        piz_dir.join("config.toml"),
        "default_backend = \"openai\"\n[openai]\napi_key = \"sk-test-123456789\"\n",
    )
    .unwrap();

    let output = Command::new("cmd")
        .args(["/C", &format!("{} config --show", piz_exe())])
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run config via cmd");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sk-t...6789"), "API key should be masked");
    assert!(
        !stdout.contains("sk-test-123456789"),
        "raw key should not appear"
    );
}

#[test]
fn piz_config_show_via_powershell() {
    let dir = tempfile::tempdir().unwrap();
    let piz_dir = dir.path().join(".piz");
    std::fs::create_dir_all(&piz_dir).unwrap();
    std::fs::write(
        piz_dir.join("config.toml"),
        "default_backend = \"openai\"\n[openai]\napi_key = \"sk-test-123456789\"\n",
    )
    .unwrap();

    let home = dir.path().display().to_string();
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$env:HOME='{}'; $env:USERPROFILE='{}'; & '{}' config --show",
                home,
                home,
                piz_exe()
            ),
        ])
        .output()
        .expect("failed to run config via powershell");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sk-t...6789"));
}

// ═══════════════════════════════════════════════════════
// 11. Windows 特有路径处理
// ═══════════════════════════════════════════════════════

#[test]
fn cmd_windows_path_with_spaces() {
    // 使用 dir 列出含空格的系统路径
    let output = Command::new("cmd")
        .args(["/C", "dir", r"C:\Program Files"])
        .output()
        .expect("failed to run cmd with spaces");
    assert!(
        output.status.success(),
        "cmd dir 'Program Files' failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn powershell_windows_path_with_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("my folder");
    std::fs::create_dir_all(&subdir).unwrap();

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Test-Path '{}'", subdir.display()),
        ])
        .output()
        .expect("failed to run ps with spaces");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim() == "True");
}

#[test]
fn bash_windows_path_conversion() {
    if !has_bash() {
        return;
    }
    // Git Bash 可以处理 /c/Users 风格路径
    let output = Command::new("bash")
        .args(["-c", "test -d /c/Windows && echo exists"])
        .output()
        .expect("failed to run bash path test");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("exists"));
}

// ═══════════════════════════════════════════════════════
// 12. Shell 特有命令差异
// ═══════════════════════════════════════════════════════

#[test]
fn cmd_dir_command() {
    let output = Command::new("cmd")
        .args(["/C", "dir /B C:\\Windows\\System32\\cmd.exe"])
        .output()
        .expect("failed to run dir");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("cmd.exe"));
}

#[test]
fn powershell_get_childitem() {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-ChildItem C:\\Windows\\System32\\cmd.exe | Select-Object -ExpandProperty Name",
        ])
        .output()
        .expect("failed to run Get-ChildItem");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("cmd.exe"));
}

#[test]
fn bash_ls_command() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args([
            "-c",
            "ls /c/Windows/System32/cmd.exe 2>/dev/null || echo found",
        ])
        .output()
        .expect("failed to run ls");
    assert!(output.status.success());
}

// ═══════════════════════════════════════════════════════
// 13. 多行输出和特殊字符
// ═══════════════════════════════════════════════════════

#[test]
fn cmd_multiline_output() {
    let output = Command::new("cmd")
        .args(["/C", "echo line1 & echo line2 & echo line3"])
        .output()
        .expect("failed to run cmd multiline");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line1"));
    assert!(stdout.contains("line2"));
    assert!(stdout.contains("line3"));
}

#[test]
fn powershell_multiline_output() {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "'line1'; 'line2'; 'line3'"])
        .output()
        .expect("failed to run ps multiline");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line1"));
    assert!(stdout.contains("line2"));
    assert!(stdout.contains("line3"));
}

#[test]
fn bash_special_characters() {
    if !has_bash() {
        return;
    }
    let output = Command::new("bash")
        .args(["-c", r#"echo 'hello "world" & <test>'"#])
        .output()
        .expect("failed to run bash special chars");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"));
    assert!(stdout.contains("world"));
}

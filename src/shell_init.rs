use anyhow::Result;

/// Generate shell integration code for the given shell.
/// The generated code defines a `piz` wrapper function that:
/// 1. Calls the piz binary in --eval mode
/// 2. Reads the confirmed command from ~/.piz/eval_command
/// 3. Evals it in the current shell (so cd, export, etc. work)
pub fn generate_init(shell: &str) -> Result<String> {
    let piz_dir = crate::config::piz_dir()?;
    let eval_file = piz_dir.join("eval_command");
    let eval_path = eval_file.display().to_string();

    let code = match shell.to_lowercase().as_str() {
        "bash" | "zsh" => generate_bash_zsh(&eval_path),
        "fish" => generate_fish(&eval_path),
        "powershell" | "pwsh" => generate_powershell(&eval_path),
        "cmd" => generate_cmd_hint(),
        other => anyhow::bail!(
            "Unsupported shell: {}. Supported: bash, zsh, fish, powershell, cmd",
            other
        ),
    };

    Ok(code)
}

fn generate_bash_zsh(eval_path: &str) -> String {
    // Use /dev/tty for UI so stdin/stdout stay clean
    format!(
        r#"# piz shell integration (bash/zsh)
# Add to your ~/.bashrc or ~/.zshrc:
#   eval "$(piz init bash)"

piz() {{
  if [ "$1" = "init" ] || [ "$1" = "config" ] || [ "$1" = "chat" ] || \
     [ "$1" = "fix" ] || [ "$1" = "clear-cache" ] || [ "$1" = "completions" ] || \
     [ "$1" = "history" ] || [ "$1" = "update" ]; then
    command piz "$@"
    return
  fi
  command piz --eval "$@"
  local rc=$?
  if [ $rc -eq 0 ] && [ -f "{eval_path}" ]; then
    local cmd
    cmd=$(cat "{eval_path}")
    rm -f "{eval_path}"
    if [ -n "$cmd" ]; then
      eval "$cmd"
    fi
  fi
}}

# Built-in aliases for convenience
alias p='piz'
alias pf='piz fix'
alias pc='piz chat'
"#,
        eval_path = eval_path.replace('\\', "/")
    )
}

fn generate_fish(eval_path: &str) -> String {
    format!(
        r#"# piz shell integration (fish)
# Add to your ~/.config/fish/config.fish:
#   piz init fish | source

function piz
  if test (count $argv) -gt 0
    switch $argv[1]
      case init config chat fix clear-cache completions history update
        command piz $argv
        return
    end
  end
  command piz --eval $argv
  set -l rc $status
  if test $rc -eq 0 -a -f "{eval_path}"
    set -l cmd (cat "{eval_path}")
    rm -f "{eval_path}"
    if test -n "$cmd"
      eval $cmd
    end
  end
end

# Built-in aliases for convenience
alias p='piz'
alias pf='piz fix'
alias pc='piz chat'
"#,
        eval_path = eval_path.replace('\\', "/")
    )
}

fn generate_powershell(eval_path: &str) -> String {
    // PowerShell: use native path separators
    format!(
        r#"# piz shell integration (PowerShell)
# Add to your $PROFILE:
#   piz init powershell | Out-String | Invoke-Expression

function Invoke-Piz {{
  param([Parameter(ValueFromRemainingArguments)][string[]]$Args)
  $subcommands = @('init','config','chat','fix','clear-cache','completions','history','update')
  if ($Args.Count -gt 0 -and $subcommands -contains $Args[0]) {{
    & piz.exe @Args
    return
  }}
  & piz.exe --eval @Args
  if ($LASTEXITCODE -eq 0 -and (Test-Path '{eval_path}')) {{
    $cmd = Get-Content '{eval_path}' -Raw -ErrorAction SilentlyContinue
    Remove-Item '{eval_path}' -Force -ErrorAction SilentlyContinue
    if ($cmd) {{
      Invoke-Expression $cmd
    }}
  }}
}}

Set-Alias -Name piz -Value Invoke-Piz -Option AllScope -Force

# Built-in aliases for convenience
function Invoke-PizFix {{ & piz.exe fix @Args }}
function Invoke-PizChat {{ & piz.exe chat @Args }}
Set-Alias -Name p -Value Invoke-Piz -Option AllScope -Force
Set-Alias -Name pf -Value Invoke-PizFix -Option AllScope -Force
Set-Alias -Name pc -Value Invoke-PizChat -Option AllScope -Force
"#,
        eval_path = eval_path.replace('/', "\\")
    )
}

fn generate_cmd_hint() -> String {
    r#"@echo off
REM piz shell integration for cmd is limited.
REM cmd.exe does not support function definitions.
REM Use piz normally — most commands work in subprocess mode.
REM For cd commands, copy the output and run manually.
REM
REM Consider using PowerShell for full shell integration:
REM   piz init powershell | Out-String | Invoke-Expression
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_bash_contains_eval() {
        let code = generate_init("bash").unwrap();
        assert!(code.contains("eval"));
        assert!(code.contains("--eval"));
        assert!(code.contains("eval_command"));
    }

    #[test]
    fn init_zsh_same_as_bash() {
        let bash = generate_init("bash").unwrap();
        let zsh = generate_init("zsh").unwrap();
        assert_eq!(bash, zsh);
    }

    #[test]
    fn init_fish_contains_function() {
        let code = generate_init("fish").unwrap();
        assert!(code.contains("function piz"));
        assert!(code.contains("--eval"));
    }

    #[test]
    fn init_powershell_contains_alias() {
        let code = generate_init("powershell").unwrap();
        assert!(code.contains("Set-Alias"));
        assert!(code.contains("Invoke-Piz"));
        assert!(code.contains("--eval"));
    }

    #[test]
    fn init_pwsh_same_as_powershell() {
        let ps = generate_init("powershell").unwrap();
        let pwsh = generate_init("pwsh").unwrap();
        assert_eq!(ps, pwsh);
    }

    #[test]
    fn init_unknown_shell_errors() {
        assert!(generate_init("unknown").is_err());
    }

    #[test]
    fn init_case_insensitive() {
        assert!(generate_init("Bash").is_ok());
        assert!(generate_init("FISH").is_ok());
        assert!(generate_init("PowerShell").is_ok());
    }

    #[test]
    fn bash_passes_subcommands_directly() {
        let code = generate_init("bash").unwrap();
        assert!(code.contains("config"));
        assert!(code.contains("chat"));
        assert!(code.contains("command piz \"$@\""));
    }

    #[test]
    fn bash_contains_builtin_aliases() {
        let code = generate_init("bash").unwrap();
        assert!(code.contains("alias p='piz'"));
        assert!(code.contains("alias pf='piz fix'"));
        assert!(code.contains("alias pc='piz chat'"));
    }

    #[test]
    fn fish_contains_builtin_aliases() {
        let code = generate_init("fish").unwrap();
        assert!(code.contains("alias p='piz'"));
        assert!(code.contains("alias pf='piz fix'"));
        assert!(code.contains("alias pc='piz chat'"));
    }

    #[test]
    fn powershell_contains_builtin_aliases() {
        let code = generate_init("powershell").unwrap();
        assert!(code.contains("Set-Alias -Name p"));
        assert!(code.contains("Set-Alias -Name pf"));
        assert!(code.contains("Set-Alias -Name pc"));
    }
}

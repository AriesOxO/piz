use anyhow::Result;

/// Read the last command from shell history as a fallback for `piz fix`
pub fn last_history_command() -> Result<String> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    // Try zsh history first, then bash
    let candidates = [home.join(".zsh_history"), home.join(".bash_history")];

    for path in &candidates {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            if let Some(last_line) = content.lines().rev().find(|l| !l.trim().is_empty()) {
                // zsh history format: ": timestamp:0;command"
                let cmd = if last_line.starts_with(':') {
                    last_line.split_once(';').map_or(last_line, |x| x.1)
                } else {
                    last_line
                };
                return Ok(cmd.trim().to_string());
            }
        }
    }

    // Windows: try PSReadLine history
    if cfg!(target_os = "windows") {
        if let Some(appdata) = dirs::data_local_dir() {
            let ps_history = appdata
                .join("Microsoft")
                .join("Windows")
                .join("PowerShell")
                .join("PSReadLine")
                .join("ConsoleHost_history.txt");
            if ps_history.exists() {
                let content = std::fs::read_to_string(ps_history)?;
                if let Some(last) = content.lines().rev().find(|l| !l.trim().is_empty()) {
                    return Ok(last.trim().to_string());
                }
            }
        }
    }

    anyhow::bail!("Could not find shell history")
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_bash_history_last_line() {
        let content = "ls\ncd /tmp\ngit status\n";
        let last = content
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap();
        assert_eq!(last, "git status");
    }

    #[test]
    fn parse_zsh_history_with_timestamp() {
        let line = ": 1234567890:0;git push --force";
        let cmd = if line.starts_with(':') {
            line.split_once(';').map_or(line, |x| x.1)
        } else {
            line
        };
        assert_eq!(cmd.trim(), "git push --force");
    }

    #[test]
    fn parse_zsh_history_without_semicolon() {
        let line = ": 1234567890:0";
        let cmd = if line.starts_with(':') {
            line.split_once(';').map_or(line, |x| x.1)
        } else {
            line
        };
        // No semicolon → returns whole line
        assert_eq!(cmd, ": 1234567890:0");
    }

    #[test]
    fn history_skips_empty_lines() {
        let content = "ls\n\n\n\npwd\n\n";
        let last = content
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap();
        assert_eq!(last, "pwd");
    }

    #[test]
    fn history_empty_file_no_result() {
        let content = "\n\n\n";
        let last = content.lines().rev().find(|l| !l.trim().is_empty());
        assert!(last.is_none());
    }

    #[test]
    fn history_single_command() {
        let content = "echo hello\n";
        let last = content
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap();
        assert_eq!(last, "echo hello");
    }

    #[test]
    fn history_multiline_zsh() {
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
}

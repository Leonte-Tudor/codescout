use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn denied_read_prefixes() -> &'static [&'static str] {
    &[
        "~/.ssh",
        "~/.aws",
        "~/.gnupg",
        "~/.config/gh",
        "~/.netrc",
        "~/.npmrc",
        "~/.pypirc",
        "~/.docker/config.json",
        "~/.kube/config",
        "~/.git-credentials",
    ]
}

pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    ("cmd.exe", vec!["/C".to_string(), cmd.to_string()])
}

pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in cmd.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if in_quotes {
        return Err("unclosed quote".to_string());
    }
    Ok(tokens)
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;
    if status.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "taskkill failed: {}",
                String::from_utf8_lossy(&status.stderr)
            ),
        ))
    }
}

pub fn process_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if to.exists() {
        std::fs::remove_file(to)?;
    }
    std::fs::rename(from, to)
}

pub fn lsp_binary_name(base: &str) -> String {
    match base {
        "typescript-language-server"
        | "vscode-json-language-server"
        | "yaml-language-server"
        | "bash-language-server"
        | "pyright-langserver" => {
            format!("{}.cmd", base)
        }
        _ => format!("{}.exe", base),
    }
}

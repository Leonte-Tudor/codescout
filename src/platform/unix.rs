use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn denied_read_prefixes() -> &'static [&'static str] {
    &[
        "~/.ssh",
        "~/.aws",
        "~/.gnupg",
        "~/.config/gcloud",
        "~/.config/gh",
        "~/.docker/config.json",
        "~/.netrc",
        "~/.npmrc",
        "~/.kube/config",
    ]
}

pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    ("sh", vec!["-c".to_string(), cmd.to_string()])
}

pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape_next = false;

    for ch in cmd.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if !in_single => escape_next = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if in_single || in_double {
        return Err("unclosed quote".to_string());
    }
    Ok(tokens)
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

pub fn lsp_binary_name(base: &str) -> String {
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_some() {
        assert!(home_dir().is_some());
    }

    #[test]
    fn temp_dir_exists() {
        assert!(temp_dir().exists());
    }

    #[test]
    fn shell_command_uses_sh() {
        let (prog, args) = shell_command("echo hello");
        assert_eq!(prog, "sh");
        assert_eq!(args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn shell_tokenize_splits_correctly() {
        let tokens = shell_tokenize("echo 'hello world'").unwrap();
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn lsp_binary_name_unchanged() {
        assert_eq!(lsp_binary_name("rust-analyzer"), "rust-analyzer");
    }
}

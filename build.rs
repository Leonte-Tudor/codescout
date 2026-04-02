fn main() {
    // Bake the codescout git SHA into the binary at compile time.
    // Falls back to "unknown" for non-git builds (e.g. crates.io install).
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=CODESCOUT_GIT_SHA={sha}");

    // Only re-run when HEAD changes (not on every source edit).
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
}

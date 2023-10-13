#[cfg(not(feature = "version-from-git"))]
fn main() {}

#[cfg(feature = "version-from-git")]
fn main() {
    // from https://stackoverflow.com/a/44407625
    use std::process::Command;
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}

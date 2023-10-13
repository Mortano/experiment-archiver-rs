/// Returns the current version of the current executable. Without the `version` feature, this
/// uses an implementation-defined unique ID of the current build. If you want the current git
/// commit hash, enable the `version-from-git` feature!
#[cfg(not(feature = "version-from-git"))]
pub(crate) fn current_version() -> String {
    // Without access to git commit hashes, we use a unique ID of the current build
    let id = build_id::get();
    format!("Build ID {}", id.to_string())
}

#[cfg(feature = "version-from-git")]
pub(crate) fn current_version() -> String {
    let hash = env!("GIT_HASH");
    format!("git commit hash {hash}")
}

//! Stamps the binary with the commit it was built from.
//!
//! The interface is rebuilt by hand and served from a process you start
//! yourself, so "am I looking at the change I just pulled, or the build from
//! an hour ago?" is a question that comes up every single time, and a
//! rebuilt page and a stale one look alike until you know what changed.
//! Putting the commit in the page answers it at a glance.

use std::process::Command;

fn main() {
    // Re-stamp when the checkout moves to a different commit. Naming any
    // dependency turns off cargo's default "rerun on any change", so the
    // sources the script actually reads are listed too.
    for path in [
        "../../.git/HEAD",
        "../../.git/refs/heads",
        "assets/index.html",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };

    // Said from outside, if anybody is saying it. A container build has no
    // repository to read: the sources are copied in without `.git`, so every
    // git command here fails and the stamp would be "unknown" on exactly the
    // builds whose identity is hardest to check by other means. A platform that
    // knows the commit passes it in this variable, and it wins outright, because
    // something that was told is better than something that was guessed.
    if let Ok(said) = std::env::var("CARRANTA_BUILD")
        && !said.trim().is_empty()
    {
        println!("cargo:rerun-if-env-changed=CARRANTA_BUILD");
        println!("cargo:rustc-env=CARRANTA_BUILD={}", said.trim());
        return;
    }
    println!("cargo:rerun-if-env-changed=CARRANTA_BUILD");
    // A tarball with no repository still has to build.
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    // A "+" means the working tree had uncommitted changes, so the commit
    // alone does not describe what is running.
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
    println!(
        "cargo:rustc-env=CARRANTA_BUILD={commit}{}",
        if dirty { "+" } else { "" }
    );
}

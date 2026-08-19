//! Which build is serving this, resolved when the process starts.
//!
//! The stamp used to be baked in at compile time alone, by `build.rs`, from a
//! `CARRANTA_BUILD` the platform was expected to pass in as a build argument.
//! That works and is fragile in a way worth naming, because it cost an evening:
//! it needs a dashboard variable to exist, a `Dockerfile` `ARG` to receive it,
//! and the platform's git variables to be present *for that build*. Miss any
//! one and every page says `unknown` while deploy after deploy lands correctly.
//! An instrument that reads the same whether or not the thing it measures
//! happened is worse than no instrument: it does not merely fail to inform, it
//! actively misleads, and a whole evening's diagnosis was built on its silence.
//!
//! So the commit is read at *runtime* first, where a platform that knows it
//! puts it anyway, and the compile-time stamp is the fallback rather than the
//! only answer. Nothing has to be configured for this to work, which is the
//! entire point: the check that tells you whether a deploy landed must not
//! itself depend on the deploy having been configured correctly.

/// The commit this process is serving, short enough to sit in a header.
///
/// In order: an explicit `CARRANTA_BUILD` in the environment, because somebody
/// saying so outright should win; then `RAILWAY_GIT_COMMIT_SHA`, which the host
/// injects into every deploy that came from a repository; then the compile-time
/// stamp, which is the right answer on a laptop and says `unknown` only when
/// there was genuinely nobody to ask.
pub fn build() -> &'static str {
    static BUILD: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUILD.get_or_init(|| {
        for name in ["CARRANTA_BUILD", "RAILWAY_GIT_COMMIT_SHA"] {
            let Ok(said) = std::env::var(name) else {
                continue;
            };
            let said = said.trim();
            if said.is_empty() {
                continue;
            }
            return short(said);
        }
        env!("CARRANTA_BUILD").to_string()
    })
}

/// A full commit hash shortened the way `git rev-parse --short` shortens it,
/// and anything else left alone but bounded.
///
/// The host passes the whole forty characters, and forty characters of hex in
/// a page header is noise rather than information: seven is what every other
/// tool shows and what the compile-time stamp already produces, so the two
/// read alike and can be compared by eye. Anything that is not a hash is
/// somebody's own label and is theirs to choose, capped only so that a stray
/// variable cannot push the header around.
fn short(said: &str) -> String {
    let hash = said.len() == 40 && said.chars().all(|c| c.is_ascii_hexdigit());
    let keep = if hash { 7 } else { 32 };
    said.chars().take(keep).collect()
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn a_full_hash_is_shortened_the_way_git_shortens_one() {
        assert_eq!(short("1b93bb7c0ffee0ddba11deadbeef0123456789ab"), "1b93bb7");
        // Not a hash: a label somebody chose, kept as it is.
        assert_eq!(short("container"), "container");
        assert_eq!(short("v2.1-rc3"), "v2.1-rc3");
        // Forty characters that are not hex are not a hash either.
        assert_eq!(short(&"z".repeat(40)), "z".repeat(32));
        // And nothing can push the header around.
        assert!(short(&"a".repeat(500)).chars().count() <= 32);
    }
}

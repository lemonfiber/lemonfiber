//! The identities the bundled rows of the diagnostics register already hold.
//!
//! Its own file rather than a list at the top of the module above, because that list
//! is read by something rather than merely written down — the guard below reads the
//! modules that emit findings and holds the two to each other — and the pair belongs
//! together.

/// Every identity a bundled check reports a finding against.
///
/// Published, because a plugin contributing a row to this register has to be able to
/// be refused for colliding with one — and naming what was collided with takes knowing
/// it. Two rules keep a collision from happening at all: a contributed identity is
/// namespaced with the declaring plugin's id, and none of these carries a colon. The
/// second rule exists because the first is a property of two naming conventions
/// staying disjoint, which is a rule with an undefended edge.
///
/// **A name ending in a dot is a family, and everything under it is taken.** Four of
/// these are: an account, an indexer, a curating service and a credential each get a
/// finding of their own, and how many there are is the operator's configuration rather
/// than this build's. The rest are whole names.
///
/// Held to the checks themselves by the guard below, which reads the modules that emit
/// them: a list nobody compares against the code is a list that goes stale in the one
/// direction that matters, leaving a renamed check's old name free for a contribution
/// to take.
pub const BUNDLED_CHECKS: &[&str] = &[
    "config.credential-permissions",
    "config.download-client",
    "config.household-telling",
    "credentials.",
    "credentials.indexer",
    "environment.api",
    "environment.autostart",
    "environment.compose",
    "environment.engine",
    "network.bindings",
    "providers.indexer.",
    "providers.indexers",
    "providers.usenet",
    "providers.usenet.",
    "services.quality-guides",
    "services.releases",
    "services.releases.",
    "storage.hardlinks",
    "storage.mode",
    "storage.permissions",
    "storage.quality-headroom",
    "storage.single-mount",
    "storage.space",
    "vpn.egress-match",
    "vpn.egress-sources",
    "vpn.killswitch",
    "vpn.port-forward",
    "vpn.port-forward-client",
    "vpn.tunnel",
    "vpn.tunnel-restored",
    "vpn.unprotected",
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::BUNDLED_CHECKS;
    use crate::doctor::Category;

    /// The nine families, as the identities they prefix.
    ///
    /// Read off the enumeration rather than written out, so a family added to the
    /// doctor is one this sweep starts looking for rather than one it silently
    /// ignores — which is the shape of a new check nobody would notice was unlisted.
    fn families() -> Vec<&'static str> {
        [
            Category::Environment,
            Category::Storage,
            Category::Network,
            Category::Vpn,
            Category::Credentials,
            Category::Services,
            Category::Providers,
            Category::Queue,
            Category::Config,
        ]
        .into_iter()
        .map(Category::as_str)
        .collect()
    }

    /// Every `.rs` file under the doctor's own directory, as the half that ships.
    ///
    /// Cut at the test module rather than at the first `#[cfg(test)]`, because a file
    /// declaring a test-only helper near the top would otherwise have almost
    /// everything it ships thrown away — and a sweep reading nothing looks exactly
    /// like a sweep finding nothing.
    ///
    /// This file is left out, and that is the whole reason the comparison means
    /// anything: the list above is a run of string literals in the doctor's own
    /// directory, so a sweep that read it would find every published name emitted
    /// here and report that nothing had gone stale.
    fn shipped() -> Vec<(PathBuf, String)> {
        let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/doctor");
        let mut read = Vec::new();
        let mut looking = vec![here];
        while let Some(at) = looking.pop() {
            for entry in fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    looking.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs")
                    && path.file_name().is_some_and(|named| named != "bundled.rs")
                {
                    let text = fs::read_to_string(&path).unwrap_or_default();
                    let cut = text
                        .find("\nmod tests {")
                        .or_else(|| text.find("\npub(crate) mod tests {"))
                        .unwrap_or(text.len());
                    read.push((path, text.get(..cut).unwrap_or_default().to_owned()));
                }
            }
        }
        read
    }

    /// Every identity one file emits, by the family each one begins with.
    ///
    /// A literal, because that is the only way an identity is ever written: some are
    /// whole, some are the fixed half of a `format!` whose tail is an account, an
    /// indexer or a service name. The tail is dropped and **the dot in front of it is
    /// kept**, which is what tells the two shapes apart in the published list:
    /// `services.releases` is one check, and `services.releases.` is a family with one
    /// finding per curating service under it.
    fn emitted(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for piece in text.split('"').skip(1).step_by(2) {
            let Some((family, rest)) = piece.split_once('.') else {
                continue;
            };
            if !families().contains(&family) {
                continue;
            }
            // Everything up to the first substitution. The dot in front of it is kept,
            // which is what tells the two shapes apart in the published list:
            // `services.releases` is one check, and `services.releases.` is a family
            // with one finding per curating service under it.
            let tail = rest.split('{').next().unwrap_or(rest);
            // An empty tail is a family, and only a substitution can leave one. A bare
            // `"providers."` written out in the source would be a string nothing ever
            // reports a finding against.
            if tail.is_empty() && !piece.contains('{') {
                continue;
            }
            // What is left has to look like the rest of an identity. This is the line
            // that keeps a sentence mentioning a family, or a path that happens to
            // begin with one, from reading as a check nobody can find.
            if tail.contains(|letter: char| !matches!(letter, 'a'..='z' | '0'..='9' | '-' | '.')) {
                continue;
            }
            found.insert(format!("{family}.{tail}"));
        }
        found
    }

    /// Every identity the doctor's own modules emit.
    fn every_identity() -> BTreeSet<String> {
        shipped()
            .iter()
            .flat_map(|(_, text)| emitted(text))
            .collect()
    }

    /// The sweep read the tree it is about, before anything is concluded from it.
    ///
    /// Two empty sets agree, so a reader that quietly found nothing would make the
    /// comparison below pass and mean nothing.
    #[test]
    fn the_doctor_s_own_modules_were_actually_read() {
        // Counted into names before the assertions rather than inside their messages,
        // which are evaluated only where an assertion fails — and a rendering nothing
        // runs is a rendering nothing holds to being readable.
        let files = shipped().len();
        let identities = every_identity().len();
        assert!(
            files > 10,
            "the sweep found {files} files under src/doctor, which means it is looking \
             in the wrong place"
        );
        assert!(
            identities > 20,
            "the sweep read {identities} identities out of them, which is fewer than the \
             doctor has"
        );
    }

    /// What is left after the family has to look like the rest of an identity.
    ///
    /// The line that keeps a sentence mentioning a family, or a path that begins with
    /// one, from reading as a check nobody can find — and the one nothing in the
    /// doctor's own source happens to exercise, which is exactly when a filter stops
    /// being a filter.
    #[test]
    fn a_family_followed_by_something_that_is_not_an_identity_is_not_one() {
        let read =
            emitted(r#" "storage.Space" "storage.space/two" "vpn." "vpn.{}" "storage.space" "#);
        assert_eq!(
            read,
            ["storage.space", "vpn."]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<String>>(),
            "an uppercase tail and a path separator are not identities, a family \
             written out with no substitution behind it is not one either, and the two \
             that are survive"
        );
    }

    /// Every identity the doctor emits is one a contribution can be refused for.
    ///
    /// The direction that matters. A check added without its identity here leaves that
    /// name free for a plugin to take, and the collision is then two rows reporting
    /// against one name — which reads as one check that cannot make up its mind.
    #[test]
    fn every_identity_the_doctor_emits_is_published() {
        let published: BTreeSet<&str> = BUNDLED_CHECKS.iter().copied().collect();
        let unpublished: Vec<String> = every_identity()
            .into_iter()
            .filter(|emitted| !published.contains(emitted.as_str()))
            .collect();
        assert!(
            unpublished.is_empty(),
            "these are emitted by a check and are not in BUNDLED_CHECKS, so a plugin \
             could take the name: {unpublished:?}"
        );
    }

    /// And nothing published is emitted nowhere.
    ///
    /// The other direction, which fails quietly: a name left behind by a rename
    /// reserves something no check reports against, so a plugin is refused an identity
    /// that collides with nothing.
    #[test]
    fn every_published_identity_is_one_a_check_still_emits() {
        let emitted = every_identity();
        let stale: Vec<&&str> = BUNDLED_CHECKS
            .iter()
            .filter(|check| !emitted.contains(**check))
            .collect();
        assert!(
            stale.is_empty(),
            "these are published as occupied and no check emits them, so a contribution \
             is refused a name nothing holds: {stale:?}"
        );
    }

    /// The sweep can fail, which is the half a passing sweep says nothing about.
    #[test]
    fn a_name_no_check_emits_is_what_the_sweep_refuses() {
        let emitted = every_identity();
        assert!(!emitted.contains("storage.nothing-emits-this"));
        assert!(emitted.contains("storage.space"));
        // And the two shapes are told apart, which is what the kept dot is for.
        assert!(emitted.contains("services.releases"));
        assert!(emitted.contains("services.releases."));
        // And the two shapes are told apart, which is what the kept dot is for.
        assert!(emitted.contains("services.releases"));
        assert!(emitted.contains("services.releases."));
    }

    /// A bundled identity never carries a colon, which is what makes a namespaced
    /// contribution unable to express a collision in the first place.
    #[test]
    fn no_bundled_identity_is_namespaced() {
        let namespaced: Vec<&&str> = BUNDLED_CHECKS
            .iter()
            .filter(|check| check.contains(':'))
            .collect();
        assert!(namespaced.is_empty(), "{namespaced:?}");
    }

    /// The list is sorted and holds each name once, so reading it is reading an index.
    #[test]
    fn the_published_list_is_sorted_and_holds_each_name_once() {
        let mut sorted = BUNDLED_CHECKS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, BUNDLED_CHECKS.to_vec());
    }
}

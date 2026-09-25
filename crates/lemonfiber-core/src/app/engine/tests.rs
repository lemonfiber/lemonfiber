use super::settling::costs;
use lemonfiber_manifest::Manifest;

const STACK: &str = include_str!("../../../../../assets/media-stack/stack.toml");

/// What the stack this repository ships would say about these services.
fn said(waiting: &[&str]) -> Option<String> {
    let named: Vec<String> = waiting.iter().map(|id| (*id).to_owned()).collect();
    Manifest::from_toml(STACK)
        .ok()
        .map(|manifest| costs(&manifest, &named))
}

/// A key the service adopts at first start is minted where none is recorded.
#[tokio::test]
async fn a_key_the_service_adopts_is_minted_before_it_starts() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("mint");
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/tmp\n");

    let settings = crate::config::Settings {
        env_file: Some(env.clone()),
        ..crate::config::Settings::default()
    };
    let ctx = crate::test_support::a_context()
        .settings(settings)
        .build()
        .with_random(std::sync::Arc::new(
            lemonfiber_fixtures::support::FixedRandom(Some(vec![7; 32])),
        ));
    let written = Manifest::from_toml(STACK).ok().map(|manifest| {
        super::mint_adopted_secrets(&ctx, &manifest);
        std::fs::read_to_string(&env).unwrap_or_default()
    });
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        written
            .as_deref()
            .is_some_and(|written| written.contains("BINDERY_API_KEY=")),
        "no key was minted for the service that adopts one: {written:?}"
    );
}

/// Both ways of starting mint the key, because there are two.
///
/// A start can be waited on or streamed, and the streamed one is what the command
/// line uses — so a credential minted on only the waited-on path is one the
/// operator never gets. That is not hypothetical: it was written on one path
/// first, and the feature did nothing at all while its own tests passed.
///
/// Pinned by the call rather than by behaviour, since neither path can be run here
/// without a container to start.
#[test]
fn both_ways_of_starting_mint_the_key_a_service_adopts() {
    const WAITED: &str = include_str!("../engine.rs");
    const STREAMED: &str = include_str!("../engine/streaming.rs");
    for (path, source) in [("engine.rs", WAITED), ("engine/streaming.rs", STREAMED)] {
        let before_spawn = source
            .split_once("mint_adopted_secrets(ctx, &manifest)")
            .map(|(before, _)| before);
        assert!(
            before_spawn.is_some(),
            "{path} starts services without minting the key one of them adopts"
        );
    }
}

/// Both ways of starting ask the same two things first, because there are two.
///
/// The same hazard the minting above is pinned against, on two questions asked at
/// moments nobody is watching. A start recorded on only the waited-on path leaves
/// the next boot bringing back whatever form the *other* path last named; a data
/// location proven present on only that path leaves the one an operator actually
/// types building a second library on the system disk.
///
/// Pinned by the call rather than by behaviour, since neither path can be run here
/// without a container to start. The production half of each file is what is read,
/// so the name appearing in this very test does not satisfy it.
#[test]
fn both_ways_of_starting_ask_the_same_things_before_they_spawn() {
    const WAITED: &str = include_str!("../engine.rs");
    const STREAMED: &str = include_str!("../engine/streaming.rs");
    for (path, source) in [("engine.rs", WAITED), ("engine/streaming.rs", STREAMED)] {
        // Split at the test module rather than at the first `#[cfg(test)]`: one of
        // these files carries a test-only re-export near its imports, and splitting
        // there would read nine lines of `use` and call them the whole file.
        let production = source
            .split_once("mod tests {")
            .map_or(source, |(before, _)| before);
        assert!(
            production.contains("autostart::noted(ctx, "),
            "{path} starts services without recording what was asked for"
        );
        assert!(
            production.contains("grounded::grounded(ctx, "),
            "{path} starts services without proving the data location is there"
        );
    }
}

/// The stack this repository ships declares the service whose key is minted for it.
///
/// The minting is gated on that declaration, so a stack that stopped naming it
/// would silently stop minting — and the service would generate a key of its own
/// into a database, which is the state nothing outside it can recover from.
#[test]
fn the_shipped_stack_declares_the_service_whose_key_is_minted() {
    let manifest = Manifest::from_toml(STACK).ok();
    let declared = manifest.is_some_and(|manifest| {
        manifest.services.iter().any(|service| {
            service
                .api
                .as_ref()
                .is_some_and(|api| api.kind == lemonfiber_manifest::ApiKind::Bindery)
        })
    });
    assert!(declared, "the shipped stack names no service with that API");
}

#[test]
fn what_a_service_is_for_is_said_in_the_stacks_own_words() {
    assert_eq!(
        said(&["jellyfin"]).as_deref(),
        Some("What that costs, while it lasts: jellyfin — Files on disk, no way to watch them."),
        "the manifest's sentence, not one written here"
    );
}

#[test]
fn several_services_are_said_together_in_the_order_the_stack_declares_them() {
    let both = said(&["seerr", "jellyfin"]);
    assert!(
        both.as_ref().is_some_and(|said| {
            said.find("jellyfin")
                .zip(said.find("seerr"))
                .is_some_and(|(jellyfin, seerr)| jellyfin < seerr)
        }),
        "asked for in one order, reported in the stack's: {both:?}"
    );
}

/// A stack that says nothing about a service contributes nothing, rather than a
/// sentence with a hole in it. Reached here by naming a service the stack does
/// not declare, which is the same silence as one that describes itself as "".
#[test]
fn a_service_the_stack_says_nothing_about_costs_no_words() {
    assert_eq!(
        said(&["not-a-service-this-stack-declares"]).as_deref(),
        Some("")
    );
}

//! What a part moved in place of the stack's own service carries over from it: the key
//! that service wrote or the stack published, and the account set up on it.

use super::*;

/// The stack's own service the moving manifest's upstream moves in place of.
const RESERVED: &str = "jellyfin";

/// The key the stack published for [`RESERVED`].
const RECORDED: &str = "recorded-by-the-stack";

/// The seeded manifest, its upstream moving in place of [`RESERVED`].
fn moving() -> Option<lemonfiber_plugin::Manifest> {
    lemonfiber_plugin::Manifest::from_toml(&seeded().replacen(
        "id          = \"komga\"\nname        = \"Komga\"\nimage",
        &format!(
            "id          = \"komga\"\nname        = \"Komga\"\nreplaces    = \"{RESERVED}\"\nimage"
        ),
        1,
    ))
    .ok()
}

/// What minting the moving manifest leaves held, `trusted` as first-party, in a stack that
/// published [`RECORDED`] for [`RESERVED`].
fn minted_moving(name: &str, trusted: &[crate::plugin::first_party::FirstParty]) -> Option<String> {
    let ctx = seeding(name);
    let manifest = moving()?;
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let recorded = format!(
        "{}{}={RECORDED}\n",
        std::fs::read_to_string(&env).unwrap_or_default(),
        crate::seed::run::published_as(RESERVED)
    );
    let _ = std::fs::write(&env, recorded);
    let mut would = crate::plugin::Installed::of(&manifest);
    would.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    would.tree = crate::test_support::A_TREE.to_owned();

    super::super::super::minting::minted(&ctx, &would, &manifest, "1", trusted).ok()?;
    minted(&ctx)
}

#[test]
fn a_first_party_part_moved_in_place_keeps_the_key_the_stack_published_for_it() {
    assert!(moving().is_some());
    assert_eq!(
        minted_moving(
            "seeded-moved-trusted",
            &crate::test_support::first_party("komga")
        )
        .as_deref(),
        Some(RECORDED)
    );
    let stranger = minted_moving("seeded-moved-stranger", &[]).unwrap_or_default();
    assert_eq!(stranger.len(), 32, "{stranger:?}");
    assert_ne!(stranger, RECORDED);
}

/// A part the bundle installs in place of the stack's own service of its id, and the file
/// beneath that service's configuration directory its declaration names as holding its
/// key.
struct MovingIn {
    /// The part's plugin, as the bundle pins it.
    moved: Option<crate::plugin::Installed>,
    /// The file the stack's own service writes its key to.
    file: &'static str,
    /// That file as the service writes it, holding [`WRITTEN`].
    holding: &'static str,
}

/// The key the stack's own service wrote into its configuration.
const WRITTEN: &str = "written-by-the-service";

/// The subtitle finder, whose stack service writes its key into a YAML.
fn subtitles() -> MovingIn {
    MovingIn {
        moved: crate::test_support::subtitles::moved(),
        file: "config/config.yaml",
        holding: "auth:\n  apikey: written-by-the-service\nsonarr:\n  apikey: someone-elses\n",
    }
}

/// The Usenet client, whose stack service writes its key into an INI.
fn usenet() -> MovingIn {
    MovingIn {
        moved: crate::test_support::usenet::moved(),
        file: "sabnzbd.ini",
        holding: "[misc]\nnzb_key = someone-elses\napi_key = written-by-the-service\n",
    }
}

/// A curator or the indexer, whose stack service writes its key into an XML: `moved` is
/// its part as the bundle pins it.
fn written_xml(moved: Option<crate::plugin::Installed>) -> MovingIn {
    MovingIn {
        moved,
        file: "config.xml",
        holding:
            "<Config>\n  <Port>8989</Port>\n  <ApiKey>written-by-the-service</ApiKey>\n</Config>\n",
    }
}

/// The request service, whose stack service writes its key into a JSON.
fn requests() -> MovingIn {
    MovingIn {
        moved: crate::test_support::requests::REQUESTS.installed(),
        file: "settings.json",
        holding: r#"{"main":{"apiKey":"written-by-the-service","applicationTitle":"Seerr"}}"#,
    }
}

/// Every part this build moves whose stack service wrote its own key, each with what it
/// is called in a failure.
fn every_written() -> [(&'static str, MovingIn); 7] {
    use crate::test_support::curators::{FILM, MUSIC, TELEVISION};
    [
        ("subtitles", subtitles()),
        ("usenet", usenet()),
        ("television", written_xml(TELEVISION.installed())),
        ("film", written_xml(FILM.installed())),
        ("music", written_xml(MUSIC.installed())),
        (
            "indexer",
            written_xml(crate::test_support::indexer::INDEXER.installed()),
        ),
        ("requests", requests()),
    ]
}

/// How the stack's own service left its key file before the move.
enum Left {
    /// Written as the service writes it.
    Written,
    /// Not there at all.
    Absent,
    /// A link leading out of the directory its container owns.
    LeadingAway,
}

/// What minting `part`'s plugin, first-party as this build embeds it, leaves held for its
/// upstream's key, in a stack whose own service of its id left its key file as `left`
/// says, and which recorded `recorded` as the key it published for it, or none.
fn minted_moving_in(
    name: &str,
    part: &MovingIn,
    left: &Left,
    recorded: Option<&str>,
) -> Option<String> {
    let ctx = seeding(name);
    let would = part.moved.clone()?;
    let plugin = would.plugin.clone();
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    if let Some(key) = recorded {
        let _ = crate::config::store::set(&env, &crate::seed::run::published_as(&plugin), key);
    }
    let at = crate::app::targets::service_config_dir(&stack_of(&ctx), &plugin).join(part.file);
    let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
    match left {
        Left::Written => {
            let _ = std::fs::write(&at, part.holding);
        }
        Left::Absent => {}
        Left::LeadingAway => {
            let elsewhere =
                lemonfiber_fixtures::scratch::Scratch::new(&format!("{name}-elsewhere")).kept();
            let _ = std::fs::create_dir_all(&elsewhere);
            let target = elsewhere.join("key");
            let _ = std::fs::write(&target, part.holding);
            link(&target, &at);
        }
    }
    let manifest = lemonfiber_bundled::PINNED
        .iter()
        .find(|one| one.first_party.plugin == plugin)
        .and_then(|one| lemonfiber_plugin::Manifest::from_toml(one.copy).ok())?;

    super::super::super::minting::minted(
        &ctx,
        &would,
        &manifest,
        "1",
        crate::plugin::first_party::EMBEDDED,
    )
    .ok()?;
    crate::settings::secrets_at(&ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|held| {
            held.value(&crate::settings::named(&plugin, "upstream-key"))
                .map(str::to_owned)
        })
}

/// `at` made a link to `target`.
#[cfg(unix)]
fn link(target: &std::path::Path, at: &std::path::Path) {
    let _ = std::os::unix::fs::symlink(target, at);
}

/// `at` left as it is where links cannot be made.
#[cfg(not(unix))]
fn link(_target: &std::path::Path, _at: &std::path::Path) {}

/// A part moved in place of the stack's own service is handed the key that service wrote
/// into its own configuration, over a stale one the stack published for it: the YAML the
/// subtitle finder writes, the INI the Usenet client writes, the XML each curator and the
/// indexer write and the JSON the request service writes alike.
#[test]
fn a_part_moved_in_place_keeps_the_key_its_service_wrote_over_the_one_published() {
    for (tag, part) in every_written() {
        assert!(part.moved.is_some(), "{tag}");
        assert_eq!(
            minted_moving_in(
                &format!("seeded-written-{tag}"),
                &part,
                &Left::Written,
                Some(RECORDED)
            )
            .as_deref(),
            Some(WRITTEN),
            "{tag}"
        );
    }
}

/// A part whose service's key file is not there, or leads out of the directory its
/// container owns, is handed the key the stack published for it instead, and nothing is
/// read through the link.
#[test]
fn a_key_file_absent_or_leading_away_hands_on_the_key_the_stack_published() {
    for (part, moving) in every_written() {
        let mut left = vec![("absent", Left::Absent)];
        if cfg!(unix) {
            left.push(("leading-away", Left::LeadingAway));
        }
        for (tag, left) in left {
            assert_eq!(
                minted_moving_in(
                    &format!("seeded-published-{part}-{tag}"),
                    &moving,
                    &left,
                    Some(RECORDED)
                )
                .as_deref(),
                Some(RECORDED),
                "{part} {tag}"
            );
        }
    }
}

/// A part whose service wrote no key and had none published for it is minted one.
#[test]
fn a_part_moved_in_place_of_a_service_with_no_key_anywhere_is_minted_one() {
    let minted =
        minted_moving_in("seeded-nothing", &usenet(), &Left::Absent, None).unwrap_or_default();

    assert_eq!(minted.len(), 32, "{:?}", minted.len());
    assert_ne!(minted, WRITTEN);
}

/// The seeded manifest, its adapter handing its upstream an account lemonfiber mints and
/// its upstream moving in place of the stack's own service `reserved`.
fn moving_an_account(reserved: &str) -> Option<lemonfiber_plugin::Manifest> {
    let account = "upstream    = { username = { secret = \"upstream-username\" }, password = \
                   { secret = \"upstream-password\" } }";
    let text = seeded()
        .replace(
            "upstream    = { api_key = { secret = \"upstream-key\" } }",
            account,
        )
        .replace(
            "id     = \"upstream-key\"\nof     = \"komga\"\nminted = true\nwhy    = \"The key the \
             adapter signs in to Komga with\"\n",
            "id     = \"upstream-username\"\nof     = \"komga\"\nminted = true\nwhy    = \"The \
             account the adapter signs in to Komga as\"\n\n[[secret]]\nid     = \
             \"upstream-password\"\nof     = \"komga\"\nminted = true\nwhy    = \"That \
             account's password\"\n",
        )
        .replacen(
            "id          = \"komga\"\nname        = \"Komga\"\nimage",
            &format!(
                "id          = \"komga\"\nname        = \"Komga\"\nreplaces    = \
                 \"{reserved}\"\nimage"
            ),
            1,
        );
    lemonfiber_plugin::Manifest::from_toml(&text).ok()
}

/// The account minting the moving-an-account manifest leaves held for its upstream moving
/// in place of the stack's own service `reserved`, `trusted` as first-party, in a stack
/// that recorded `recorded` as its torrent client's password, or none.
fn minted_account(
    name: &str,
    reserved: &str,
    recorded: Option<&str>,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Option<(String, String)> {
    let ctx = seeding(name);
    let manifest = moving_an_account(reserved)?;
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let _ = match recorded {
        Some(password) => {
            crate::config::store::set(&env, crate::config::TORRENT_PASSWORD_KEY, password)
        }
        None => crate::config::store::unset(&env, crate::config::TORRENT_PASSWORD_KEY),
    };
    let mut would = crate::plugin::Installed::of(&manifest);
    would.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    would.tree = crate::test_support::A_TREE.to_owned();

    super::super::super::minting::minted(&ctx, &would, &manifest, "1", trusted).ok()?;
    let held =
        crate::settings::secrets_at(&ctx).and_then(|at| crate::settings::Secrets::at(&at).ok())?;
    let part = |id: &str| held.value(&format!("komga:{id}")).map(str::to_owned);
    Some((part("upstream-username")?, part("upstream-password")?))
}

/// A first-party upstream moved in place of the stack's torrent client keeps the account
/// lemonfiber set up on it, by the API the stack declares that client answering in: the
/// name the stack signs in under, and the password recorded for it.
#[test]
fn a_first_party_part_moved_in_place_keeps_the_account_the_stack_set_up_on_it() {
    assert!(moving_an_account("qbittorrent").is_some());
    let trusted = crate::test_support::first_party("komga");

    let kept = minted_account(
        "seeded-account-trusted",
        "qbittorrent",
        Some(RECORDED),
        &trusted,
    );
    assert_eq!(kept, Some(("admin".to_owned(), RECORDED.to_owned())));

    let stranger = minted_account(
        "seeded-account-stranger",
        "qbittorrent",
        Some(RECORDED),
        &[],
    )
    .unwrap_or_default();
    assert_eq!((stranger.0.len(), stranger.1.len()), (32, 32));
    assert_ne!(stranger.1, RECORDED);
}

/// An upstream moved in place of a stack service lemonfiber sets up no account on, or of
/// one the stack does not declare, has a fresh account minted, whatever the stack
/// recorded.
#[test]
fn a_part_moved_in_place_of_a_service_with_no_account_is_minted_one() {
    for reserved in ["sonarr", "undeclared"] {
        let minted = minted_account(
            &format!("seeded-account-none-{reserved}"),
            reserved,
            Some(RECORDED),
            &crate::test_support::first_party("komga"),
        )
        .unwrap_or_default();

        assert_eq!((minted.0.len(), minted.1.len()), (32, 32), "{reserved}");
        assert_ne!(minted.1, RECORDED, "{reserved}");
    }
}

/// A password the stack never recorded for the service a part moves in place of is minted
/// afresh, while the name the stack signs in under is still kept.
#[test]
fn a_part_moved_in_place_of_a_service_with_no_password_recorded_is_minted_one() {
    let minted = minted_account(
        "seeded-account-unrecorded",
        "qbittorrent",
        None,
        &crate::test_support::first_party("komga"),
    )
    .unwrap_or_default();

    assert_eq!(minted.0, "admin");
    assert_eq!(minted.1.len(), 32, "{:?}", minted.1.len());
}

/// Every secret minting `moved`, a part the bundle installs in place of the stack's own
/// service of its id, leaves held, by secret id, in a stack that recorded each setting of
/// `recorded` with its value.
fn carried(
    name: &str,
    moved: Option<crate::plugin::Installed>,
    recorded: &[(&str, &str)],
) -> std::collections::BTreeMap<String, String> {
    let ctx = seeding(name);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    for (setting, value) in recorded {
        let _ = crate::config::store::set(&env, setting, value);
    }
    let Some(would) = moved else {
        return std::collections::BTreeMap::new();
    };
    let manifest = lemonfiber_bundled::PINNED
        .iter()
        .find(|one| one.first_party.plugin == would.plugin)
        .and_then(|one| lemonfiber_plugin::Manifest::from_toml(one.copy).ok());
    let minted = manifest.as_ref().and_then(|manifest| {
        super::super::super::minting::minted(
            &ctx,
            &would,
            manifest,
            "1",
            crate::plugin::first_party::EMBEDDED,
        )
        .ok()
    });
    let held = crate::settings::secrets_at(&ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .filter(|_| minted.is_some())
        .unwrap_or_default();
    manifest
        .iter()
        .flat_map(|manifest| manifest.secrets.iter())
        .filter_map(|secret| {
            held.value(&crate::settings::named(&would.plugin, &secret.id))
                .map(|value| (secret.id.clone(), value.to_owned()))
        })
        .collect()
}

/// The Usenet indexer aggregator moved in place keeps the administrator lemonfiber turned
/// its authentication on for, and the listening server the root account lemonfiber made:
/// each the name the stack signed in under, with the password it recorded.
#[test]
fn the_aggregator_and_the_listening_server_moved_in_place_keep_their_accounts() {
    let aggregator = carried(
        "seeded-carried-aggregator",
        crate::test_support::aggregator::MOVED.installed(),
        &[(
            crate::config::USENET_AGGREGATOR_ADMIN_PASSWORD_KEY,
            RECORDED,
        )],
    );
    let listening = carried(
        "seeded-carried-listening",
        crate::test_support::listening::MOVED.installed(),
        &[(crate::config::LISTENING_SERVER_PASSWORD_KEY, RECORDED)],
    );

    let held =
        |minted: &std::collections::BTreeMap<String, String>, id: &str| minted.get(id).cloned();
    assert_eq!(
        (
            held(&aggregator, "admin-username"),
            held(&aggregator, "admin-password")
        ),
        (Some("admin".to_owned()), Some(RECORDED.to_owned()))
    );
    assert_eq!(
        (
            held(&listening, "root-username"),
            held(&listening, "root-password")
        ),
        (Some("admin".to_owned()), Some(RECORDED.to_owned()))
    );
}

/// The book curator moved in place keeps the key lemonfiber recorded and handed it through
/// its environment, and is minted the first account it never had.
#[test]
fn the_book_curator_moved_in_place_keeps_its_key_and_is_minted_an_account() {
    let minted = carried(
        "seeded-carried-books",
        crate::test_support::books::moved(),
        &[(crate::config::BOOK_CURATOR_API_KEY, RECORDED)],
    );

    assert_eq!(minted.get("api-key").map(String::as_str), Some(RECORDED));
    for id in ["admin-username", "admin-password"] {
        let fresh = minted.get(id).cloned().unwrap_or_default();
        assert_eq!(fresh.len(), 32, "{id}: {fresh:?}");
        assert_ne!(fresh, RECORDED, "{id}");
    }
}

/// A part moved in place of a stack service whose password was never recorded is minted
/// one, keeping the name the stack signs in under.
#[test]
fn a_moved_aggregator_whose_password_was_never_recorded_is_minted_one() {
    let minted = carried(
        "seeded-carried-unrecorded",
        crate::test_support::aggregator::MOVED.installed(),
        &[],
    );

    assert_eq!(
        minted.get("admin-username").map(String::as_str),
        Some("admin")
    );
    assert_eq!(
        minted.get("admin-password").map(String::len),
        Some(32),
        "{minted:?}"
    );
}

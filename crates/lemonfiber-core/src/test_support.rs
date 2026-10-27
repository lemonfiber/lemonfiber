//! The fixtures that name a type from this crate.
//!
//! Everything a test stands in for lives in `lemonfiber-fixtures`, and the context a test
//! drives a command through lives in `lemonfiber-testing`; both are reached from here.
//! What is written here names a type of this crate that neither of those can.

pub(crate) use lemonfiber_fixtures::support::*;

/// The context a command is driven through, the same file the testing crate is built from:
/// the core's tests cannot depend on a crate depending on the core.
#[path = "../../lemonfiber-testing/src/context.rs"]
pub mod context;

pub(crate) use context::{a_context, declaring, nowhere, repository_stack as stack};

#[path = "../../lemonfiber-testing/src/adapted.rs"]
pub mod adapted;
#[path = "../../lemonfiber-testing/src/aggregator.rs"]
pub mod aggregator;
#[path = "../../lemonfiber-testing/src/books.rs"]
pub mod books;
#[path = "../../lemonfiber-testing/src/curators.rs"]
pub mod curators;
#[path = "../../lemonfiber-testing/src/indexer.rs"]
pub mod indexer;
#[path = "../../lemonfiber-testing/src/listening.rs"]
pub mod listening;
#[path = "../../lemonfiber-testing/src/media.rs"]
pub mod media;
#[path = "../../lemonfiber-testing/src/requests.rs"]
pub mod requests;
#[path = "../../lemonfiber-testing/src/subtitles.rs"]
pub mod subtitles;
#[path = "../../lemonfiber-testing/src/torrent.rs"]
pub mod torrent;
#[path = "../../lemonfiber-testing/src/usenet.rs"]
pub mod usenet;

mod crossing;
pub(crate) use crossing::{a_crossing_reader, a_grant, a_trusted_crossing_reader};
mod gating;
pub(crate) use gating::{a_contracted_gate, unaskable_gates, GATE_TRUSTED};

/// The day the fixture clock is stopped on, as the manifest's date rules read it.
///
/// One day for every test that validates the real manifest, read from the same
/// seconds `Stopped::today()` stops on, so it moves when that does. A day that will
/// not convert falls back to the epoch, which every recorded release is after, so
/// the tests that use it fail rather than pass on a day nobody chose.
pub(crate) fn frozen_day() -> lemonfiber_manifest::Date {
    let seconds = i64::try_from(lemonfiber_fixtures::ports::TODAY).unwrap_or_default();
    lemonfiber_manifest::Date::from_unix_seconds(seconds).unwrap_or(lemonfiber_manifest::Date {
        year: 1970,
        month: 1,
        day: 1,
    })
}

/// The overlay's changes the journal holds, oldest first, each as the fingerprints it
/// went from and to.
pub(crate) fn overlaid(ctx: &crate::app::Ctx) -> Vec<(Option<String>, Option<String>)> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .filter_map(|change| match &change.kind {
                    crate::journal::Kind::Overlaid {
                        previous, current, ..
                    } => Some((previous.clone(), current.clone())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A journal line for a setting written over nothing — a fresh file, so the prior value
/// is absent.
///
/// Three test modules each built this, because the journal's shape is what they assert
/// against and stating it independently is the point: a test that echoed how apply
/// computes a change would pass whatever apply did. Stating it once does not weaken that
/// — what matters is that the expectation is written out rather than derived, not that
/// it is written out three times.
pub(crate) fn a_fresh_write(key: &str, value: &str) -> crate::journal::Change {
    crate::journal::Change {
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: crate::journal::Kind::Set {
            key: key.to_owned(),
            previous: None,
            current: value.to_owned(),
        },
    }
}

/// The same file with nothing recorded in it, for a test about anything but the
/// password.
///
/// Built rather than written at the call site: an empty string literal beside a password
/// key reads to a secret scanner as a hard-coded credential, and a fixture that has to
/// be argued about every time it is scanned is worse than one that says what it means.
#[track_caller]
pub(crate) fn env_without_password(name: &str) -> std::path::PathBuf {
    recorded(name, None)
}

/// A private env file recording the stack's torrent client's password, at a scratch path
/// unique to the test so concurrent tests do not share one.
///
/// It names `config::store`, which is why it is on this side of the fixtures boundary.
#[track_caller]
pub(crate) fn env_at(name: &str, password: &str) -> std::path::PathBuf {
    recorded(name, Some(password))
}

/// The file itself, with a password where there is one.
///
/// The absence is `None` rather than an empty string, so a test that wants no password
/// says so instead of writing a blank one — which reads to a secret scanner as a
/// hard-coded credential and to a reader as a password that happens to be empty.
#[track_caller]
fn recorded(name: &str, password: Option<&str>) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("env-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        crate::config::store::set(
            &path,
            crate::config::TORRENT_PASSWORD_KEY,
            password.unwrap_or_default(),
        )
        .is_ok(),
        "the scratch env file is written"
    );
    path
}

/// One installed plugin's service, filling `provides`, speaking `api` where it names one
/// and answering on `listens` where it says.
///
/// Its configuration directory is the format's fallback, `/config`, so an adapter path
/// beneath it reads the way a bundled service's does.
pub(crate) fn a_placed(
    service: &str,
    provides: &[&str],
    api: Option<lemonfiber_manifest::Api>,
    listens: Option<u16>,
) -> crate::plugin::Placed {
    crate::plugin::Placed {
        fields: lemonfiber_manifest::container::Fields::default(),
        service: service.to_owned(),
        image: "example.invalid/stand-in".to_owned(),
        digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
            .to_owned(),
        tag: "1.0.0".to_owned(),
        config_path: "/config".to_owned(),
        takes_data: false,
        reached: listens.map(|port| crate::plugin::Reached::Loopback { port, group: None }),
        provides: provides.iter().map(|one| (*one).to_owned()).collect(),
        name: format!("{service} the stand-in"),
        description: "Stands in for a bundled service".to_owned(),
        icon: None,
        hidden: false,
        health: None,
        api,
        listens,
        media_types: Vec::new(),
        networks: Vec::new(),
        speaks: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: None,
        replaces: None,
        shape: None,
        asks: Vec::new(),
        reaches: None,
        asks_for: None,
        starting: None,
    }
}

/// The service a plugin brings in place of the stack's own request service.
pub(crate) const CONTRACTED_REQUESTS: &str = "requests";

/// Where the host reaches [`CONTRACTED_REQUESTS`].
pub(crate) const CONTRACTED_REQUESTS_AT: &str =
    "http://127.0.0.1:8080/lemonfiber/request.intake/v1/";

/// A context over the shipped stack without its own request service, beside a plugin
/// whose [`CONTRACTED_REQUESTS`] fills `request.intake` and speaks its first major on
/// loopback 8080, holding its key where `keyed`, with the media server moved into its
/// plugin, its administrator's password recorded and every call answered by `http`.
pub(crate) fn beside_contracted_requests(
    tag: &str,
    keyed: bool,
    http: std::sync::Arc<lemonfiber_fixtures::http::Fake>,
) -> crate::app::Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("contracted-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    lemonfiber_fixtures::stack::manifest_without(&dir, "seerr");
    if keyed {
        let at = crate::plugin::key_file(&dir, CONTRACTED_REQUESTS);
        let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
        let _ = std::fs::write(&at, "contracted-key");
    }
    let engine = Reporting::holding(
        &[CONTRACTED_REQUESTS],
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    )
    .publishing(&[(CONTRACTED_REQUESTS, "127.0.0.1", 8080)])
    .alongside(media::running());
    let mut ctx = a_context()
        .over(crate::stack::Source::External(Box::leak(
            dir.clone().into_boxed_path(),
        )))
        .engine(std::sync::Arc::new(engine))
        .build()
        .with_http(http);
    ctx.settings.env_file = Some(dir.join(".env"));
    let _ = crate::app::targets::record_secret(
        &ctx,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    let mut placed = a_placed(CONTRACTED_REQUESTS, &["request.intake"], None, Some(8080));
    placed.speaks = vec!["request.intake@1".to_owned()];
    let mut register = crate::plugin::Register::empty();
    let _ = register.record(an_installed("intake", vec![placed]));
    if let Some(kept) = crate::app::plugins::kept_at(&ctx) {
        let _ = std::fs::write(kept, register.to_json().unwrap_or_default());
    }
    media::moved_in(&ctx);
    ctx
}

/// `ctx` with every part the bundle installs recorded beside an env file of its own under
/// `name`.
pub(crate) fn parts_recorded(mut ctx: crate::app::Ctx, name: &str) -> crate::app::Ctx {
    ctx.settings.env_file = Some(env_without_password(name));
    adapted::parts_recorded(&ctx);
    ctx
}

/// A curator configuration carrying a readable key, so the stack resolves its targets.
pub(crate) const KEYED_CURATOR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// A context over the repository's stack with its media server moved into its plugin,
/// every curator keyed, an env file of its own under `name`, and every call answered by
/// `http`.
pub(crate) fn moved_household(
    name: &str,
    http: std::sync::Arc<lemonfiber_fixtures::http::Fake>,
) -> crate::app::Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("moved-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context()
        .engine(std::sync::Arc::new(media::running()))
        .build()
        .with_filesystem(std::sync::Arc::new(SeedFs::keyed(Some(KEYED_CURATOR))))
        .with_http(http);
    ctx.settings.env_file = Some(dir.join(".env"));
    media::moved_in(&ctx);
    ctx
}

/// The name the secret store holds the key lemonfiber minted for [`a_minting_curator`]'s
/// upstream under.
pub(crate) const A_MINTED_KEY: &str = "series:upstream-key";

/// A curator plugin: its adapter `series` provides `library.curate` and fronts its
/// upstream `series-upstream`, which talks `servarr` and holds the key lemonfiber minted
/// for it as [`A_MINTED_KEY`].
pub(crate) fn a_minting_curator() -> crate::plugin::Installed {
    let mut adapter = a_placed("series", &["library.curate"], None, None);
    adapter.fronts = Some("series-upstream".to_owned());
    let mut upstream = a_placed("series-upstream", &[], None, Some(8989));
    upstream.native = Some("servarr".to_owned());
    let mut curator = an_installed("series", vec![adapter, upstream]);
    curator.declared.secrets = vec![crate::plugin::Secret {
        id: "upstream-key".to_owned(),
        of: "series-upstream".to_owned(),
        why: "The key the adapter signs in with".to_owned(),
        minted: true,
        serves: None,
    }];
    curator
}

/// An installed plugin holding these services.
pub(crate) fn an_installed(
    plugin: &str,
    services: Vec<crate::plugin::Placed>,
) -> crate::plugin::Installed {
    crate::plugin::Installed {
        granted: Vec::new(),
        plugin: plugin.to_owned(),
        version: "1.0.0".to_owned(),
        name: None,
        description: None,
        services,
        provides: Vec::new(),
        contributions: Vec::new(),
        run_proofs: Vec::new(),
        request_proofs: Vec::new(),
        declared: crate::plugin::Declaration::default(),
        recipes: Vec::new(),
        adapters: Vec::new(),
        from: String::new(),
        revision: String::new(),
        signed: String::new(),
        manifest: String::new(),
        tree: String::new(),
        installed_at: String::new(),
    }
}

/// `installed` carrying the digests the bundle pins its plugin at, so this build holds it
/// first-party.
pub(crate) fn pinned_as(mut installed: crate::plugin::Installed) -> crate::plugin::Installed {
    if let Some(pinned) = lemonfiber_bundled::PINNED
        .iter()
        .find(|one| one.first_party.plugin == installed.plugin)
    {
        pinned
            .first_party
            .manifest
            .clone_into(&mut installed.manifest);
        pinned.first_party.tree.clone_into(&mut installed.tree);
    }
    installed
}

/// The digest a contracted plugin was installed from.
pub(crate) const CONTRACTED_MANIFEST: &str = "contracted-manifest";

/// The digest of the tree a first-party plugin was installed from.
pub(crate) const A_TREE: &str = "sha256:a-tree";

/// The port a contracted plugin's service speaks on, inside its container and on the
/// host's loopback alike.
pub(crate) const CONTRACTED_PORT: u16 = 8080;

/// The key a contracted plugin's service holds in a context built for it.
pub(crate) const CONTRACTED_KEY: &str = "contracted-key";

/// The adapter [`a_contracted_media_server`] brings.
pub(crate) const MEDIA_ADAPTER: &str = "media-adapter";

/// The upstream [`MEDIA_ADAPTER`] fronts.
pub(crate) const MEDIA_UPSTREAM: &str = "media-upstream";

/// A plugin bringing a contracted media server: [`MEDIA_ADAPTER`], speaking
/// `identity.source` and `media.serve` on [`CONTRACTED_PORT`] in front of
/// [`MEDIA_UPSTREAM`], which names `native` as the API it answers in where one is given.
pub(crate) fn a_contracted_media_server(native: Option<&str>) -> crate::plugin::Installed {
    let mut adapter = a_placed(
        MEDIA_ADAPTER,
        &["identity.source", "media.serve"],
        None,
        Some(CONTRACTED_PORT),
    );
    adapter.speaks = vec!["identity.source@1".to_owned(), "media.serve@1".to_owned()];
    adapter.fronts = Some(MEDIA_UPSTREAM.to_owned());
    let mut upstream = a_placed(MEDIA_UPSTREAM, &[], None, Some(9000));
    upstream.native = native.map(str::to_owned);
    let mut installed = an_installed("contracted", vec![adapter, upstream]);
    installed.manifest = CONTRACTED_MANIFEST.to_owned();
    installed.tree = crate::test_support::A_TREE.to_owned();
    installed
}

/// A plugin whose service `service` provides `capability` and speaks its first major on
/// [`CONTRACTED_PORT`].
pub(crate) fn contracted(
    plugin: &str,
    service: &str,
    capability: &str,
) -> crate::plugin::Installed {
    let mut placed = a_placed(service, &[capability], None, Some(CONTRACTED_PORT));
    placed.speaks = vec![format!("{capability}@1")];
    let mut installed = an_installed(plugin, vec![placed]);
    installed.manifest = CONTRACTED_MANIFEST.to_owned();
    installed.tree = crate::test_support::A_TREE.to_owned();
    installed
}

/// `plugin`, installed from its contracted manifest, as first-party.
pub(crate) const fn first_party(
    plugin: &'static str,
) -> [crate::plugin::first_party::FirstParty; 1] {
    [crate::plugin::first_party::FirstParty {
        plugin,
        manifest: CONTRACTED_MANIFEST,
        tree: A_TREE,
    }]
}

/// A context reaching `service` on loopback [`CONTRACTED_PORT`], holding its key in
/// `project` where `keyed`, beside the media server's adapter running where a moved server
/// is reached.
pub(crate) fn contracted_context(
    project: &std::path::Path,
    service: &str,
    keyed: bool,
) -> context::Context {
    if keyed {
        let at = crate::plugin::key_file(project, service);
        let _ = std::fs::create_dir_all(at.parent().unwrap_or(&at));
        let _ = std::fs::write(&at, CONTRACTED_KEY);
    }
    let engine = Reporting::holding(
        &[service],
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    )
    .publishing(&[(service, "127.0.0.1", CONTRACTED_PORT)])
    .alongside(media::running());
    a_context().engine(std::sync::Arc::new(engine))
}

/// The id of the request service `server` is handed to, where one is.
pub(crate) fn asker(server: &crate::app::targets::MediaServer) -> Option<&str> {
    server.asked_by.as_ref().map(|asker| asker.id.as_str())
}

/// The shipped stack's fillers with `services` as its services and `installed` beside
/// them, written at `project`, trusting `trusted` as first-party.
pub(crate) fn stack_fillers(
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    project: Option<&std::path::Path>,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = services;
            crate::wiring::Fillers::trusting(
                &manifest,
                installed,
                &crate::wiring::Chosen::default(),
                project,
                trusted,
            )
        })
        .unwrap_or_default()
}

/// `services` in the order the embedded stack declares them, where it runs every one.
pub(crate) fn in_the_stacks_order(services: &[&str]) -> Option<Vec<String>> {
    lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .ok()
        .and_then(|manifest| {
            let ordered: Vec<String> = manifest
                .services
                .into_iter()
                .map(|service| service.id)
                .filter(|id| services.contains(&id.as_str()))
                .collect();
            (ordered.len() == services.len()).then_some(ordered)
        })
}

/// A value as a contract carries it.
pub(crate) fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The stack's egress guard running healthy, its status file reading `port`.
pub(crate) fn a_guard_forwarding(port: Option<&'static str>) -> std::sync::Arc<Reporting> {
    std::sync::Arc::new(
        Reporting::holding(
            &["gluetun"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::Healthy,
        )
        .with_tunnel(Tunnel {
            gateway: "gluetun",
            gateway_ip: None,
            client_ip: None,
            country: None,
            port,
            second_opinion: None,
        }),
    )
}

mod tests;

/// The stack this build carries, assembled from its bundle and written apart as the
/// binary embeds it.
pub(crate) fn carried_stack() -> crate::stack::Source {
    use include_dir::{Dir, DirEntry, File};
    let leaked = |text: String| -> &'static str { Box::leak(text.into_boxed_str()) };
    let (root, files) =
        lemonfiber_manifest::assembly::split(lemonfiber_bundled::STACK).unwrap_or_default();
    let services: Vec<DirEntry<'static>> = files
        .into_iter()
        .map(|(file, text)| {
            DirEntry::File(File::new(
                leaked(format!(
                    "{}/{file}",
                    lemonfiber_manifest::assembly::SERVICES
                )),
                leaked(text).as_bytes(),
            ))
        })
        .collect();
    let entries: Vec<DirEntry<'static>> = vec![
        DirEntry::File(File::new(
            lemonfiber_manifest::assembly::ROOT,
            leaked(root).as_bytes(),
        )),
        DirEntry::Dir(Dir::new(
            lemonfiber_manifest::assembly::SERVICES,
            Box::leak(services.into_boxed_slice()),
        )),
    ];
    crate::stack::Source::Embedded(Box::leak(Box::new(Dir::new(
        "",
        Box::leak(entries.into_boxed_slice()),
    ))))
}

/// The version of the stack this build ships.
pub(crate) fn shipped_stack_version() -> String {
    lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .map(|manifest| manifest.stack_version)
        .unwrap_or_default()
}

/// The stack this build carries inside it, as a machine that
/// has chosen no stack directory yet holds it.
pub(crate) fn embedded() -> crate::stack::Source {
    crate::stack::Source::Embedded(&lemonfiber_bundled::CARRIED)
}

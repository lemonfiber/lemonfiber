//! A start creates again only the containers whose entry apart from its networks changed
//! or that were on a network created again, within what it reaches, and brings every other
//! container onto its service's networks where it runs.

use std::collections::BTreeMap;
use std::sync::Arc;

use lemonfiber_fixtures::networked::Networked;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::{refused, spoke, Recording, Reporting};
use lemonfiber_ports::docker::{Endpoint, Health, Lifecycle, ENTRY_LABEL};

use crate::app::Ctx;
use crate::test_support::Holds as _;

/// The project with `tv` waiting for `db`, both on `a`, and `web` on `a` and `b`.
const FIRST: &str = "\
services:
  tv:
    image: tv:1
    depends_on:
      db:
        condition: service_started
    networks: [a]
  db:
    image: db:1
    networks: [a]
  web:
    image: web:1
    environment:
      PORT: '80'
    networks: [a, b]
networks:
  a:
    name: lemonfiber_a
    external: true
  b:
    name: lemonfiber_b
    external: true
  c:
    name: lemonfiber_c
    external: true
";

/// The project with `crawler` inside the network of `fence`, and `helper` inside `crawler`'s.
const FENCED: &str = "\
services:
  fence:
    image: fence:1
    networks: [a]
  crawler:
    image: crawler:1
    network_mode: service:fence
    depends_on: [fence]
  helper:
    image: helper:1
    network_mode: service:crawler
networks:
  a:
    name: lemonfiber_a
    external: true
";

/// What a container runs before the start: its service, its label and its networks.
type Running<'a> = (&'a str, Option<&'a str>, &'a [&'a str]);

/// A machine whose project is `document`, whose containers run as `running`, whose engine
/// connects and disconnects as `networks` does and whose program running the project answers
/// as `runner` does.
fn machine(
    name: &str,
    document: &str,
    running: &[Running<'_>],
    (networks, runner): (&Arc<Networked>, &Arc<Recording>),
) -> Ctx {
    overlaid(
        name,
        document,
        running,
        (networks, runner),
        (Vec::new(), Pulled::unreachable("no engine")),
    )
}

/// The same machine, with the operator's `overlays` layered over its project and the images
/// its references name as `images` says.
fn overlaid(
    name: &str,
    document: &str,
    running: &[Running<'_>],
    (networks, runner): (&Arc<Networked>, &Arc<Recording>),
    (overlays, images): (Vec<std::path::PathBuf>, Arc<Pulled>),
) -> Ctx {
    let dir = Scratch::new(&format!("converging-{name}"));
    let _ = std::fs::write(dir.join(crate::project::FILE), document);
    let services: Vec<&str> = running.iter().map(|(service, ..)| *service).collect();
    let engine = running.iter().fold(
        Reporting::holding(&services, Lifecycle::Running, Health::Healthy),
        |engine, (service, entry, on)| engine.labelled(service, *entry, on),
    );
    crate::test_support::a_context()
        .runner(runner.clone())
        .engine(Arc::new(engine))
        .networks(networks.clone())
        .images(images)
        .settings(crate::config::Settings {
            overlays,
            ..crate::config::Settings::default()
        })
        .stack_in(dir)
        .build()
}

/// A program running the project that does what it is told.
fn obeying() -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(spoke(""))))
}

/// Run what a start of `scope` takes away first over `ctx`; the services each command it ran
/// was aimed at.
async fn renewed(
    ctx: &Ctx,
    scope: &[&str],
    displaced: &[&str],
    runner: &Recording,
) -> Vec<Vec<String>> {
    let scope: Vec<String> = scope.iter().map(|one| (*one).to_owned()).collect();
    let displaced: Vec<String> = displaced.iter().map(|one| (*one).to_owned()).collect();
    let _ = super::renewed(ctx, &scope, &displaced, |gone| {
        std::iter::once("rm".to_owned()).chain(gone).collect()
    })
    .await;
    runner.seen()
}

/// Each service's digest, as the labels file the start wrote holds it.
fn digests(ctx: &Ctx) -> BTreeMap<String, String> {
    let text = crate::app::project::directory(ctx)
        .and_then(|directory| std::fs::read_to_string(directory.join(crate::project::LABELS)).ok())
        .unwrap_or_default();
    let labels: serde_norway::Value = serde_norway::from_str(&text).unwrap_or_default();
    labels
        .get("services")
        .and_then(serde_norway::Value::as_mapping)
        .into_iter()
        .flatten()
        .filter_map(|(service, entry)| {
            let digest = entry.get("labels")?.get(ENTRY_LABEL)?.as_str()?;
            Some((service.as_str()?.to_owned(), digest.to_owned()))
        })
        .collect()
}

/// The digests the project `document` gives its services, read on a machine of the name
/// `name` of its own.
async fn digested(name: &str, document: &str) -> BTreeMap<String, String> {
    let ctx = machine(name, document, &[], (&Networked::none(), &obeying()));
    let _ = super::renewed(&ctx, &[], &[], |_| Vec::new()).await;
    digests(&ctx)
}

/// The digest `digests` holds for `service`.
fn of<'a>(digests: &'a BTreeMap<String, String>, service: &str) -> Option<&'a str> {
    digests.get(service).map(String::as_str)
}

#[tokio::test]
async fn installs_adding_networks_to_running_services_restart_none_of_them() {
    let first = digested("installs_adding_networks-first", FIRST).await;
    let second_document = FIRST.replace(
        "    networks: [a]\n  db:",
        "    networks:\n      a:\n      b:\n        aliases: [shows]\n  db:",
    );
    let third_document = second_document.replace(
        "    networks: [a, b]\n",
        "    networks:\n      a:\n      b:\n      c:\n        ipv4_address: 10.80.0.4\n",
    );
    let networks = Networked::none();
    let runner = obeying();
    let running = [
        ("tv", of(&first, "tv"), &["lemonfiber_a"][..]),
        ("db", of(&first, "db"), &["lemonfiber_a"][..]),
        (
            "web",
            of(&first, "web"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
    ];
    let second = machine("second", &second_document, &running, (&networks, &runner));
    let ran_second = renewed(&second, &[], &[], &runner).await;
    let running = [
        (
            "tv",
            of(&first, "tv"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
        ("db", of(&first, "db"), &["lemonfiber_a"][..]),
        (
            "web",
            of(&first, "web"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
    ];
    let third = machine("third", &third_document, &running, (&networks, &runner));
    let ran_third = renewed(&third, &[], &[], &runner).await;

    assert!(ran_second.is_empty(), "{ran_second:?}");
    assert!(ran_third.is_empty(), "{ran_third:?}");
    assert_eq!(digests(&third), first);
    assert_eq!(
        networks.joined(),
        [
            "+lemonfiber_b id-tv tv,shows -",
            "+lemonfiber_c id-web web 10.80.0.4",
        ]
    );
}

#[tokio::test]
async fn a_changed_entry_creates_again_only_its_own_service() {
    let first = digested("a_changed_entry_creates_-first", FIRST).await;
    let runner = obeying();
    let running = [
        ("tv", of(&first, "tv"), &["lemonfiber_a"][..]),
        ("db", of(&first, "db"), &["lemonfiber_a"][..]),
        (
            "web",
            of(&first, "web"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
    ];
    let changed = FIRST.replace("PORT: '80'", "PORT: '81'");
    let ctx = machine("changed", &changed, &running, (&Networked::none(), &runner));

    let ran = renewed(&ctx, &[], &[], &runner).await;

    assert_eq!(ran, [["rm", "web"]]);
    assert_ne!(of(&digests(&ctx), "web"), of(&first, "web"));
}

#[tokio::test]
async fn a_service_that_was_on_a_network_created_again_is_created_again_alone() {
    let first = digested("a_service_that_was_on_a_-first", FIRST).await;
    let runner = obeying();
    let running = [
        ("tv", of(&first, "tv"), &[][..]),
        ("db", of(&first, "db"), &["lemonfiber_a"][..]),
        (
            "web",
            of(&first, "web"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
    ];
    let networks = Networked::none();
    let ctx = machine("displaced", FIRST, &running, (&networks, &runner));

    let ran = renewed(&ctx, &[], &["tv"], &runner).await;

    assert_eq!(ran, [["rm", "tv"]]);
    assert!(networks.joined().is_empty(), "{:?}", networks.joined());
}

#[tokio::test]
async fn a_container_leaves_each_network_of_the_project_its_service_no_longer_joins() {
    let first = digested("a_container_leaves_each_-first", FIRST).await;
    let runner = obeying();
    let running = [(
        "db",
        of(&first, "db"),
        &["bridge", "lemonfiber_a", "lemonfiber_c"][..],
    )];
    let networks = Networked::none();
    let ctx = machine("leaving", FIRST, &running, (&networks, &runner));

    let ran = renewed(&ctx, &[], &[], &runner).await;

    assert!(ran.is_empty(), "{ran:?}");
    assert_eq!(networks.joined(), ["-lemonfiber_c id-db"]);
}

#[tokio::test]
async fn a_start_creates_again_only_what_it_reaches_through_what_each_waits_for() {
    let runner = obeying();
    let running = [
        ("tv", Some("old"), &["lemonfiber_a"][..]),
        ("db", None, &["lemonfiber_a"][..]),
        ("web", Some("old"), &["lemonfiber_a", "lemonfiber_b"][..]),
    ];
    let ctx = machine("reached", FIRST, &running, (&Networked::none(), &runner));

    let ran = renewed(&ctx, &["tv"], &["web"], &runner).await;

    assert_eq!(ran, [["rm", "db", "tv"]]);
}

#[tokio::test]
async fn a_container_the_engine_will_not_connect_is_created_again_where_the_start_reaches_it() {
    let first = digested("a_container_the_engine_w-first", FIRST).await;
    let runner = obeying();
    let running = [
        ("tv", of(&first, "tv"), &[][..]),
        ("db", of(&first, "db"), &[][..]),
        ("web", of(&first, "web"), &[][..]),
    ];
    let ctx = machine(
        "unjoined",
        FIRST,
        &running,
        (&Networked::unjoining(&[]), &runner),
    );

    let ran = renewed(&ctx, &["web"], &[], &runner).await;

    assert_eq!(ran, [["rm", "web"]]);
}

#[tokio::test]
async fn a_fenced_container_that_will_not_go_refuses_the_start() {
    let runner = Arc::new(Recording::answering(Ok(refused("no such container"))));
    let running = [
        ("fence", None, &["lemonfiber_a"][..]),
        ("crawler", None, &[][..]),
    ];
    let ctx = machine("stuck", FENCED, &running, (&Networked::none(), &runner));

    let renewed = super::renewed(&ctx, &["crawler".to_owned()], &[], |gone| {
        std::iter::once("rm".to_owned()).chain(gone).collect()
    })
    .await;

    assert_eq!(runner.seen(), [["rm", "crawler", "fence"]]);
    assert_eq!(
        renewed.map_err(|problem| (problem.code, problem.detail)),
        Err((
            crate::error::codes::stack::CONTAINER_NOT_RENEWED,
            Some("no such container".to_owned())
        ))
    );
}

#[tokio::test]
async fn a_service_inside_a_fence_created_again_is_created_again_with_it_through_a_chain() {
    let fenced = digested("inside-fenced", FENCED).await;
    let changed = FENCED.replace("image: fence:1", "image: fence:2");
    let runner = obeying();
    let running = [
        ("fence", of(&fenced, "fence"), &["lemonfiber_a"][..]),
        ("crawler", of(&fenced, "crawler"), &[][..]),
        ("helper", of(&fenced, "helper"), &[][..]),
        ("tv", None, &["lemonfiber_a"][..]),
    ];
    let ctx = machine("inside", &changed, &running, (&Networked::none(), &runner));

    let ran = renewed(&ctx, &["fence"], &[], &runner).await;
    let after = digests(&ctx);

    assert_eq!(ran, [["rm", "crawler", "fence", "helper"]]);
    assert_ne!(of(&after, "helper"), of(&fenced, "helper"));
}

#[tokio::test]
async fn a_service_inside_a_fence_that_was_displaced_goes_with_it() {
    let fenced = digested("inside-moved-fenced", FENCED).await;
    let runner = obeying();
    let running = [
        ("fence", of(&fenced, "fence"), &["lemonfiber_a"][..]),
        ("crawler", of(&fenced, "crawler"), &[][..]),
        ("helper", of(&fenced, "helper"), &[][..]),
    ];
    let ctx = machine(
        "inside-moved",
        FENCED,
        &running,
        (&Networked::none(), &runner),
    );

    let ran = renewed(&ctx, &["fence"], &["fence"], &runner).await;

    assert_eq!(ran, [["rm", "crawler", "fence", "helper"]]);
}

#[tokio::test]
async fn an_overlay_and_a_secrets_file_are_part_of_what_a_container_is_created_from() {
    let with_secrets = FIRST.replace(
        "    image: db:1\n",
        "    image: db:1\n    env_file: [db.env, {path: more.env, required: false}]\n",
    );
    let before = digested("an_overlay_and_a_secrets-before", &with_secrets).await;
    let dir = Scratch::new("converging-overlay-file");
    let overlay = dir.join("overlay.yml");
    let _ = std::fs::write(
        &overlay,
        "services:\n  tv:\n    mem_limit: 1g\n    networks: [c]\n",
    );
    let ctx = overlaid(
        "overlaid",
        &with_secrets,
        &[],
        (&Networked::none(), &obeying()),
        (vec![overlay], Pulled::unreachable("no engine")),
    )
    .holding(dir);
    let directory = crate::app::project::directory(&ctx).unwrap_or_default();
    let _ = std::fs::write(directory.join("db.env"), "KEY=secret\n");

    let renewed = super::renewed(&ctx, &[], &[], |_| Vec::new()).await;
    let after = digests(&ctx);

    assert!(renewed.is_ok());

    assert_ne!(of(&after, "tv"), of(&before, "tv"));
    assert_ne!(of(&after, "db"), of(&before, "db"));
    assert_eq!(of(&after, "web"), of(&before, "web"));
}

#[tokio::test]
async fn a_machine_with_no_project_written_creates_nothing_again() {
    let runner = obeying();
    let ctx = crate::test_support::a_context()
        .runner(runner.clone())
        .build();

    let ran = renewed(&ctx, &[], &["tv"], &runner).await;

    assert!(ran.is_empty());
}

#[tokio::test]
async fn an_image_its_reference_names_anew_creates_again_only_the_containers_running_another() {
    let first = digested("an_image_its_reference_n-first", FIRST).await;
    let runner = obeying();
    let running = [
        ("tv", of(&first, "tv"), &["lemonfiber_a"][..]),
        ("db", of(&first, "db"), &["lemonfiber_a"][..]),
        (
            "web",
            of(&first, "web"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
    ];
    let images = Pulled::identifying(&[("tv:1", "image-tv-pulled-again"), ("db:1", "image-db")]);
    let ctx = overlaid(
        "imaged",
        FIRST,
        &running,
        (&Networked::none(), &runner),
        (Vec::new(), images),
    );

    let ran = renewed(&ctx, &[], &[], &runner).await;

    assert_eq!(ran, [["rm", "tv"]]);
}

#[tokio::test]
async fn a_machine_with_nowhere_to_write_a_project_creates_nothing_again() {
    let runner = obeying();
    let ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .runner(runner.clone())
        .build();

    let ran = renewed(&ctx, &[], &["tv"], &runner).await;

    assert!(ran.is_empty());
}

#[tokio::test]
async fn labels_that_cannot_be_written_are_said_and_the_start_goes_on() {
    let heard = Arc::new(lemonfiber_fixtures::heard::Heard::default());
    let ctx = machine("unlabelled", FIRST, &[], (&Networked::none(), &obeying()))
        .with_narrator(heard.clone());
    let directory = crate::app::project::directory(&ctx).unwrap_or_default();
    assert!(std::fs::create_dir_all(directory.join(crate::project::LABELS)).is_ok());

    let renewed = super::renewed(&ctx, &[], &[], |_| Vec::new()).await;

    assert!(renewed.is_ok());
    assert!(
        heard
            .said()
            .iter()
            .any(|said| said.starts_with("the labels of the project could not be written")),
        "{:?}",
        heard.said()
    );
}

#[tokio::test]
async fn a_service_naming_no_image_is_kept_while_its_entry_holds() {
    const BUILT: &str = "\
services:
  tool:
    build: .
    depends_on: db
  db:
networks:
  plain: {}
";
    let first = digested("a_service_naming_no_imag-first", BUILT).await;
    let runner = obeying();
    let running = [("tool", of(&first, "tool"), &[][..])];
    let images = Pulled::identifying(&[("tool", "image-tool")]);
    let ctx = overlaid(
        "built",
        BUILT,
        &running,
        (&Networked::none(), &runner),
        (Vec::new(), images),
    );

    let ran = renewed(&ctx, &["tool"], &[], &runner).await;

    assert!(of(&first, "db").is_some());
    assert!(ran.is_empty(), "{ran:?}");
}

#[tokio::test]
async fn a_secrets_file_named_alone_is_part_of_what_a_container_is_created_from() {
    let alone = FIRST.replace(
        "    image: db:1\n",
        "    image: db:1\n    env_file: db.env\n",
    );
    let ctx = machine("alone", &alone, &[], (&Networked::none(), &obeying()));
    let _ = super::renewed(&ctx, &[], &[], |_| Vec::new()).await;
    let before = digests(&ctx);
    let directory = crate::app::project::directory(&ctx).unwrap_or_default();
    let _ = std::fs::write(directory.join("db.env"), "KEY=secret\n");

    let _ = super::renewed(&ctx, &[], &[], |_| Vec::new()).await;
    let after = digests(&ctx);

    assert_ne!(of(&after, "db"), of(&before, "db"));
    assert_eq!(of(&after, "web"), of(&before, "web"));
}

/// How a container stands on `network`: answering to `aliases`, holding `address`.
fn endpoint(network: &str, aliases: &[&str], address: Option<&str>) -> Endpoint {
    Endpoint {
        network: network.to_owned(),
        address: address.map(str::to_owned),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
    }
}

#[tokio::test]
async fn a_joined_network_whose_names_or_address_changed_is_joined_again_and_no_other() {
    let document = FIRST.replace(
        "    networks: [a, b]\n",
        "    networks:\n      a:\n        aliases: [site]\n      b:\n        ipv4_address: 10.80.0.5\n",
    );
    let first = digested("a_joined_network_whose_n-first", &document).await;
    let runner = obeying();
    let running = [
        ("db", of(&first, "db"), &["lemonfiber_a"][..]),
        (
            "web",
            of(&first, "web"),
            &["lemonfiber_a", "lemonfiber_b"][..],
        ),
    ];
    let networks = Networked::none();
    networks.standing("id-db", endpoint("lemonfiber_a", &["db"], None));
    networks.standing("id-web", endpoint("lemonfiber_a", &["web", "www"], None));
    networks.standing(
        "id-web",
        endpoint("lemonfiber_b", &["web"], Some("10.80.0.4")),
    );
    let ctx = machine("rejoined", &document, &running, (&networks, &runner));

    let ran = renewed(&ctx, &[], &[], &runner).await;

    assert!(ran.is_empty(), "{ran:?}");
    assert_eq!(
        networks.joined(),
        [
            "-lemonfiber_a id-web",
            "+lemonfiber_a id-web web,site -",
            "-lemonfiber_b id-web",
            "+lemonfiber_b id-web web 10.80.0.5",
        ]
    );
}

#[tokio::test]
async fn a_container_whose_networks_cannot_be_read_is_created_again_where_the_start_reaches_it() {
    let first = digested("unread-networks-first", FIRST).await;
    let runner = obeying();
    let running = [(
        "web",
        of(&first, "web"),
        &["lemonfiber_a", "lemonfiber_b"][..],
    )];
    let ctx = machine(
        "unread",
        FIRST,
        &running,
        (&Networked::unlisting(), &runner),
    );

    let ran = renewed(&ctx, &[], &[], &runner).await;

    assert_eq!(ran, [["rm", "web"]]);
}

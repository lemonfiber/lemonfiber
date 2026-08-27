//! What a browser is told about lemonfiber's own files, and what it may be handed.
//!
//! The two reads that exist because a browser has no filesystem in front of it.
//! One says which backups this machine has kept, which is what a shell reads out of
//! the directory before it types a path. The other hands over a support bundle,
//! which is what a shell gets by having asked for the file to be written somewhere
//! it can already reach.
//!
//! Three properties are worth driving a request for.
//!
//! **A name is resolved, never followed.** The bundle is asked for by name and the
//! path it lands on is built by the core beneath a directory lemonfiber chose. A
//! name climbing out of that directory is refused rather than resolved — proven
//! here against a file that really is up there, because a rule about traversal that
//! is only tested against a name that reaches nothing proves nothing about
//! traversal.
//!
//! **A file is handed over as a file.** Its own type, marked as something to keep
//! rather than something to show, and with no filename quoted into a header from
//! anything a request supplied.
//!
//! **A refusal is still an envelope.** A caller that asked for something it could
//! parse asked about the refusals most of all, and the status tells a name that is
//! not here from a machine that could not answer.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::to_bytes;
use axum::http::StatusCode;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::archive::{Archive, Archiving, Fault, Reader, Space};
use lemonfiber_core::backup::{Existing, Item, Manifest};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};

/// A vault that answers with the backups a test scripted and packs nothing.
///
/// Only what a listing asks of the port is answered. Nothing here writes, unpacks
/// or measures, because neither of the two reads under test does any of that — and
/// a fake that pretended to would be a second description of what a capture is.
struct Kept(Result<Vec<Existing>, Fault>);

impl Kept {
    /// A vault keeping the backups named, each with the moment it was taken.
    fn holding(kept: &[(&str, &str)]) -> Self {
        Self(Ok(kept
            .iter()
            .map(|(name, taken)| Existing {
                name: (*name).to_owned(),
                created_at: (*taken).to_owned(),
            })
            .collect()))
    }
}

/// Nothing this fake is asked for beyond a listing is answered.
fn unasked() -> Fault {
    Fault::new("this test asks only for a listing")
}

#[async_trait]
impl Archive for Kept {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, Fault> {
        Err(unasked())
    }
    async fn write(
        &self,
        _dest: &Path,
        _manifest: &Manifest,
        _items: &[Item],
    ) -> Result<(), Fault> {
        Err(unasked())
    }
    async fn write_files(&self, _dest: &Path, _files: &[(String, String)]) -> Result<(), Fault> {
        Err(unasked())
    }
    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, Fault> {
        self.0.clone()
    }
    async fn remove(&self, _dir: &Path, _name: &str) -> Result<(), Fault> {
        Err(unasked())
    }
}

#[async_trait]
impl Reader for Kept {
    async fn read_manifest(&self, _src: &Path) -> Result<Manifest, Fault> {
        Err(unasked())
    }
    async fn extract(&self, _src: &Path, _targets: &[(String, PathBuf)]) -> Result<(), Fault> {
        Err(unasked())
    }
}

/// A scratch directory of this test's own, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-kept-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A run keeping its files under `dir`, with a vault answering `kept`.
fn ctx(dir: &Path, kept: Kept) -> lemonfiber_core::app::Ctx {
    lemonfiber_core::app::Ctx::new(
        Arc::new(Idle),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        lemonfiber_core::stack::Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings::default(),
        Environment::MacOs,
    )
    .with_random(Arc::new(Chance::cycling()))
    .keeping(Archiving {
        paths: Paths::at(dir, dir),
        vault: Arc::new(kept),
    })
}

/// The read routes as a run builds them, over a context a test chose.
fn routed(ctx: lemonfiber_core::app::Ctx) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    lemonfiber_api::read::routes().with_state(Serving {
        ctx: Arc::new(ctx),
        token: Arc::new(token),
        bound: lemonfiber_api::guard::Binding::here(8471),
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        jobs: Jobs::default(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    })
}

/// What a path answered with: its status, its headers, and its body.
async fn got(ctx: lemonfiber_core::app::Ctx, path: &str) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(path)
        .body(axum::body::Body::empty());
    let Ok(request) = request else {
        unreachable!("a request built from values that are already a URI cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(ctx), request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let body = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, headers, body)
}

/// One header of an answer, or nothing where it carried none.
fn header_of(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(given, _)| given == name)
        .map(|(_, value)| value.clone())
}

/// A directory holding one bundle of the given name and contents.
fn holding_a_bundle(test: &str, name: &str, contents: &str) -> PathBuf {
    let dir = scratch(test);
    let bundles = dir.join("support");
    assert!(
        std::fs::create_dir_all(&bundles).is_ok(),
        "the scratch directory is writable"
    );
    assert!(std::fs::write(bundles.join(name), contents).is_ok());
    dir
}

// ── Which backups this machine has kept ───────────────────────────────────────

#[tokio::test]
async fn the_backups_kept_here_are_served_newest_first() {
    let dir = scratch("listing");
    let ctx = ctx(
        &dir,
        Kept::holding(&[
            ("lemonfiber-full-1.tar.gz", "00000000000000000001"),
            ("lemonfiber-full-2.tar.gz", "00000000000000000002"),
        ]),
    );
    let (status, _, body) = got(ctx, "/api/backups").await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(status, StatusCode::OK.as_u16(), "{said}");
    assert!(said.contains(r#""kind":"archives""#), "{said}");
    assert!(
        said.contains(r#"["lemonfiber-full-2.tar.gz","lemonfiber-full-1.tar.gz"]"#),
        "the one taken last is the one wanted first: {said}"
    );
}

#[tokio::test]
async fn a_machine_that_has_kept_nothing_serves_an_empty_listing() {
    let dir = scratch("listing-empty");
    let (status, _, body) = got(ctx(&dir, Kept::holding(&[])), "/api/backups").await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(status, StatusCode::OK.as_u16(), "{said}");
    assert!(said.contains(r#""archives":[]"#), "{said}");
}

#[tokio::test]
async fn a_backups_directory_that_will_not_be_read_is_refused_rather_than_emptied() {
    let dir = scratch("listing-refused");
    let ctx = ctx(&dir, Kept(Err(Fault::new("permission denied"))));
    let (status, _, body) = got(ctx, "/api/backups").await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        "a directory that would not be read is not an empty one: {said}"
    );
    assert!(said.contains("BACKUP-7"), "{said}");
}

// ── The bundle, handed over ───────────────────────────────────────────────────

#[tokio::test]
async fn a_bundle_this_run_kept_is_handed_over_as_a_file_to_keep() {
    let dir = holding_a_bundle("bundle-held", "lemonfiber-support-1.tar.gz", "an archive");
    let (status, headers, body) = got(
        ctx(&dir, Kept::holding(&[])),
        "/api/bundle/lemonfiber-support-1.tar.gz",
    )
    .await;
    assert_eq!(status, StatusCode::OK.as_u16());
    assert_eq!(body, b"an archive".to_vec(), "the file itself, whole");
    assert_eq!(
        header_of(&headers, "content-type"),
        Some("application/gzip".to_owned())
    );
    assert_eq!(
        header_of(&headers, "content-disposition"),
        Some("attachment".to_owned()),
        "kept rather than shown, and named by the address rather than by a header"
    );
    assert_eq!(
        header_of(&headers, "x-content-type-options"),
        Some("nosniff".to_owned())
    );
    assert_eq!(
        header_of(&headers, "cache-control"),
        Some("no-store".to_owned())
    );
}

#[tokio::test]
async fn a_name_climbing_out_of_the_bundles_directory_reaches_nothing_it_climbed_to() {
    // The file really is one level up, and is still not readable through this.
    let dir = holding_a_bundle("bundle-climb", "lemonfiber-support-1.tar.gz", "an archive");
    assert!(std::fs::write(dir.join("secrets.env"), "INDEXER_KEY=live").is_ok());
    let (status, _, body) = got(
        ctx(&dir, Kept::holding(&[])),
        "/api/bundle/%2e%2e%2fsecrets.env",
    )
    .await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(
        status,
        StatusCode::NOT_FOUND.as_u16(),
        "a name that climbs is refused by name: {said}"
    );
    assert!(said.contains("BUNDLE-8"), "{said}");
    assert!(
        !said.contains("INDEXER_KEY"),
        "and nothing it climbed to is in the answer: {said}"
    );
}

#[tokio::test]
async fn a_name_that_names_no_bundle_is_absent_rather_than_broken() {
    let dir = holding_a_bundle(
        "bundle-missing",
        "lemonfiber-support-1.tar.gz",
        "an archive",
    );
    let (status, _, body) = got(
        ctx(&dir, Kept::holding(&[])),
        "/api/bundle/lemonfiber-support-9.tar.gz",
    )
    .await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16(), "{said}");
    assert!(
        said.contains(r#""kind":"error""#),
        "still an envelope: {said}"
    );
}

#[tokio::test]
async fn a_bundle_asked_for_at_a_path_of_its_own_is_no_route_at_all() {
    // A name with a separator in it is not one segment, so it never reaches the
    // handler to be refused — which is the outer half of the same rule.
    let dir = holding_a_bundle("bundle-nested", "lemonfiber-support-1.tar.gz", "an archive");
    let (status, _, _) = got(
        ctx(&dir, Kept::holding(&[])),
        "/api/bundle/older/lemonfiber-support-1.tar.gz",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND.as_u16());
}

#[tokio::test]
async fn a_run_with_nowhere_to_keep_its_files_says_so_rather_than_answering_with_nothing() {
    let bare = lemonfiber_core::app::Ctx::new(
        Arc::new(Idle),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        lemonfiber_core::stack::Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings::default(),
        Environment::MacOs,
    )
    .with_random(Arc::new(Chance::cycling()));
    let (status, _, body) = got(bare, "/api/bundle/lemonfiber-support-1.tar.gz").await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR.as_u16(), "{said}");
    assert!(said.contains("BUNDLE-7"), "{said}");
}

#[tokio::test]
async fn a_bundle_asked_for_with_a_parameter_is_refused_the_way_every_read_is() {
    // It is named by its address and takes nothing else, so a query string on it is
    // a request this surface cannot answer as it stands rather than one to ignore.
    let dir = holding_a_bundle(
        "bundle-parameter",
        "lemonfiber-support-1.tar.gz",
        "an archive",
    );
    let (status, _, body) = got(
        ctx(&dir, Kept::holding(&[])),
        "/api/bundle/lemonfiber-support-1.tar.gz?out=/etc/passwd",
    )
    .await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16(), "{said}");
    assert!(said.contains("READ-1"), "{said}");
}

#[tokio::test]
async fn a_listing_asked_for_with_a_parameter_is_refused_the_same_way() {
    let dir = scratch("listing-parameter");
    let (status, _, body) = got(ctx(&dir, Kept::holding(&[])), "/api/backups?only=full").await;
    let said = String::from_utf8(body).unwrap_or_default();
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16(), "{said}");
    assert!(said.contains("READ-1"), "{said}");
}

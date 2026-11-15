use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    AppKeys, IdentitySource, MediaServer, Protocol, Recounting, Requests, Subtitles, Watched,
};

use super::subtitles::WatchingNothing;
use super::*;

/// What the upstream is told is written down in order, and what it answers names what it
/// was asked about.
#[tokio::test]
async fn what_the_upstream_is_told_is_written_down_and_its_answers_name_what_was_asked() {
    let upstream = Upstream::default();
    let source = IdentitySource {
        at: "http://media:8096".to_owned(),
        protocol: Protocol("jellyfin".to_owned()),
        credential: None,
    };
    let watched = Watched {
        which: Kind::Tv,
        host: "curator".to_owned(),
        port: 8989,
        api_key: "key".to_owned(),
    };

    assert!(upstream.replace_admin_password("secret").await.is_ok());
    assert!(upstream.establish(&source).await.is_ok());
    assert!(upstream.watch(&watched).await.is_ok());
    assert!(upstream
        .events(Some("http://media"), None, Some("key"), Some(30))
        .await
        .is_ok_and(|heard| heard.is_some_and(|heard| heard.every == Some(30))));
    assert!(upstream
        .events(None, Some("pin"), None, None)
        .await
        .is_ok_and(|heard| heard.is_none()));
    assert_eq!(
        upstream.operations(),
        ["rotated", "establish", "watch", "events", "events"]
    );

    assert!(upstream
        .minted_as()
        .await
        .is_ok_and(|minted| minted == "Seerr"));
    assert!(upstream
        .astray("lemonfiber")
        .await
        .is_ok_and(|astray| astray.iter().all(|one| one.name == "lemonfiber")));
    assert!(upstream
        .account("lemonfiber")
        .await
        .is_ok_and(|account| account.username == "lemonfiber"));
    assert!(upstream
        .listed_as()
        .await
        .is_ok_and(|server| !server.name.is_empty()));
    assert!(WatchingNothing.watch(&watched).await.is_ok());
}

use super::{
    Accepted, Credential, File, Kept, Kind, Outcome, Record, Tokens, Upstream, Upstreams, FORMAT,
    PORT,
};
use crate::TokenHash;

fn sonarr() -> Upstream {
    Upstream {
        route: "sonarr".to_owned(),
        kind: Kind::Sonarr,
        address: "http://sonarr:8989".to_owned(),
        credential: Credential::new("the-sonarr-key"),
    }
}

#[test]
fn the_files_have_their_names_and_the_gate_its_port() {
    assert_eq!(File::Upstreams.name(), "upstreams.json");
    assert_eq!(File::Tokens.name(), "tokens.json");
    assert_eq!(File::Record.name(), "record.json");
    assert_eq!(PORT, 5057);
}

#[test]
fn upstreams_read_back_and_find_a_route_whatever_its_case() {
    let upstreams = Upstreams::of(vec![sonarr()]);
    let read = Upstreams::read(&upstreams.written());

    assert_eq!(read.as_ref().ok(), Some(&upstreams));
    assert_eq!(
        upstreams.route("SONARR").map(|one| one.kind),
        Some(Kind::Sonarr)
    );
    assert!(upstreams.route("radarr").is_none());
    assert_eq!(
        upstreams.route("sonarr").map(|one| one.credential.reveal()),
        Some("the-sonarr-key")
    );
}

#[test]
fn a_credential_is_withheld_from_debug() {
    let shown = format!("{:?}", sonarr());

    assert!(!shown.contains("the-sonarr-key"), "{shown}");
    assert!(shown.contains("withheld"), "{shown}");
}

#[test]
fn a_route_accepts_only_its_own_tokens_and_both_while_one_replaces_the_other() {
    let tokens = Tokens::of(vec![
        Accepted {
            route: "sonarr".to_owned(),
            tokens: vec![TokenHash::of("old"), TokenHash::of("new")],
        },
        Accepted {
            route: "radarr".to_owned(),
            tokens: vec![TokenHash::of("films")],
        },
    ]);

    assert!(tokens.accepts("sonarr", "old"));
    assert!(tokens.accepts("Sonarr", "new"));
    assert!(!tokens.accepts("sonarr", "films"));
    assert!(!tokens.accepts("jellyfin", "old"));
    assert_eq!(Tokens::read(&tokens.written()).ok(), Some(tokens.clone()));
    assert!(!tokens.written().contains("\"old\""));
}

#[test]
fn the_record_numbers_its_entries_and_keeps_no_query_string() {
    let record = Record::default()
        .with(
            10,
            "radarr",
            "DELETE",
            "/radarr/api/v3/movie/7?deleteFiles=true",
            Outcome::Removed,
            Kept::standard(),
        )
        .with(
            11,
            "sonarr",
            "POST",
            "/sonarr/api/v3/tag",
            Outcome::Refused,
            Kept::standard(),
        );

    assert_eq!(
        record.entries.iter().map(|one| one.seq).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        record.entries.first().map(|one| one.path.as_str()),
        Some("/radarr/api/v3/movie/7")
    );
    assert_eq!(Record::read(&record.written()).ok(), Some(record));
}

#[test]
fn the_record_keeps_only_what_it_may_and_says_what_fell_off() {
    let mut record = Record::default();
    for at in 0..1005 {
        record = record.with(
            at,
            "sonarr",
            "POST",
            "/sonarr/api/v3/tag",
            Outcome::Refused,
            Kept::standard(),
        );
    }

    assert_eq!(record.entries.len(), Kept::standard().entries());
    assert_eq!(record.entries.first().map(|one| one.seq), Some(6));
    let (unread, lost) = record.since(2);
    assert_eq!(unread.len(), 1000);
    assert_eq!(lost, 3);
    let (unread, lost) = record.since(1004);
    assert_eq!((unread.len(), lost), (1, 0));
    assert_eq!(record.since(1005), (Vec::new(), 0));
}

#[test]
fn a_file_in_another_format_or_shape_is_refused() {
    for (text, read) in [
        (
            format!(r#"{{"format":{},"upstreams":[]}}"#, FORMAT + 1),
            Upstreams::read as fn(&str) -> Result<Upstreams, crate::Unreadable>,
        ),
        ("not json".to_owned(), Upstreams::read),
    ] {
        assert!(read(&text).is_err(), "{text}");
    }
    assert!(Tokens::read(&format!(r#"{{"format":{},"routes":[]}}"#, FORMAT + 1)).is_err());
    assert!(Tokens::read("[]").is_err());
    assert!(Record::read(&format!(r#"{{"format":{},"entries":[]}}"#, FORMAT + 1)).is_err());
    assert!(Record::read("[]").is_err());
}

use super::{arrived, filed, kept, settled, Asking, Standing};
use crate::collected::{Medium, Reference, Unit};
use crate::ports::service::{BookUnit, MusicUnit};

fn path(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir.join("household-asks.json")
}

fn asking(unit: Unit, standing: Standing) -> Asking {
    Asking {
        unit,
        reference: "mbid-1".to_owned(),
        name: "Kind of Blue".to_owned(),
        by: Some("Miles Davis".to_owned()),
        year: Some(1959),
        member: "ada".to_owned(),
        at: "2026-11-08T22:00:00Z".to_owned(),
        standing,
    }
}

#[test]
fn nothing_kept_reads_as_no_asks() {
    assert_eq!(
        kept(&path("collected-none"))
            .ok()
            .map(|kept| kept.asks.len()),
        Some(0)
    );
}

#[test]
fn each_medium_numbers_its_own_asks_from_one() {
    let at = path("collected-numbers");
    let album = Unit::Music(MusicUnit::Album);
    let book = Unit::Books(BookUnit::Book);
    assert_eq!(
        filed(&at, &asking(album, Standing::Sent)).ok(),
        Some(Reference::Kept(Medium::Music, 1))
    );
    assert_eq!(
        filed(&at, &asking(book, Standing::Waiting)).ok(),
        Some(Reference::Kept(Medium::Books, 1))
    );
    assert_eq!(
        filed(&at, &asking(album, Standing::Waiting)).ok(),
        Some(Reference::Kept(Medium::Music, 2))
    );
    let asks = kept(&at).map(|kept| kept.asks).unwrap_or_default();
    assert_eq!(asks.len(), 3);
    assert_eq!(
        asks.first().map(|ask| ask.name.as_str()),
        Some("Kind of Blue")
    );
}

#[test]
fn only_a_waiting_ask_is_settled_and_only_once() {
    let at = path("collected-settled");
    let artist = Unit::Music(MusicUnit::Artist);
    let _ = filed(&at, &asking(artist, Standing::Sent));
    let waiting = filed(&at, &asking(artist, Standing::Waiting));
    assert_eq!(waiting.ok(), Some(Reference::Kept(Medium::Music, 2)));
    let sent = Reference::Kept(Medium::Music, 1);
    assert_eq!(
        settled(&at, sent, Standing::Declined, "later").ok(),
        Some(None)
    );
    let named = Reference::Kept(Medium::Music, 2);
    let was = settled(&at, named, Standing::Declined, "later");
    assert_eq!(
        was.ok().flatten().map(|ask| ask.standing),
        Some(Standing::Waiting)
    );
    assert_eq!(
        settled(&at, named, Standing::Sent, "later").ok(),
        Some(None)
    );
    let asks = kept(&at).map(|kept| kept.asks).unwrap_or_default();
    assert_eq!(
        asks.iter()
            .map(|ask| (ask.standing, ask.settled_at.as_deref()))
            .collect::<Vec<_>>(),
        [(Standing::Sent, None), (Standing::Declined, Some("later"))]
    );
}

#[test]
fn asks_kept_where_no_file_can_be_written_are_refused() {
    let at = path("asks-unwritable");
    assert!(std::fs::create_dir_all(&at).is_ok());

    let asking = asking(Unit::Music(MusicUnit::Album), Standing::Waiting);
    assert!(filed(&at, &asking).is_err());
    assert!(settled(
        &at,
        Reference::Kept(Medium::Music, 1),
        Standing::Sent,
        "2026-11-09"
    )
    .is_err());
}

#[test]
fn a_sent_ask_arrives_once_and_keeps_when_it_was_first_seen_held() {
    let at = path("collected-arrived");
    let album = Unit::Music(MusicUnit::Album);
    let named = filed(&at, &asking(album, Standing::Sent)).ok();
    let waiting = filed(&at, &asking(album, Standing::Waiting)).ok();
    let (Some(named), Some(waiting)) = (named, waiting) else {
        unreachable!("the asks were not kept");
    };

    let first = arrived(&at, named, Standing::PartlyHere, "2026-11-09T10:00:00Z");
    let again = arrived(&at, named, Standing::PartlyHere, "2026-11-09T11:00:00Z");
    let whole = arrived(&at, named, Standing::Here, "2026-11-09T12:00:00Z");
    let never = arrived(&at, waiting, Standing::Here, "2026-11-09T12:00:00Z");

    assert!(first.is_ok() && again.is_ok() && whole.is_ok() && never.is_ok());
    let asks = kept(&at).map(|kept| kept.asks).unwrap_or_default();
    let read = |reference: Reference| {
        asks.iter()
            .find(|ask| ask.named() == reference)
            .map(|ask| (ask.standing, ask.arrived_at.clone(), ask.settled_at.clone()))
    };
    assert_eq!(
        read(named),
        Some((
            Standing::Here,
            Some("2026-11-09T10:00:00Z".to_owned()),
            Some("2026-11-09T12:00:00Z".to_owned())
        ))
    );
    assert_eq!(read(waiting), Some((Standing::Waiting, None, None)));
}

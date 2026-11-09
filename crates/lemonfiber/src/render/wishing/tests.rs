use lemonfiber_core::app::Outcome;
use lemonfiber_core::asking::{Policy, Standing};
use lemonfiber_core::model::{
    Askable, AskableKind, AskableKindsReport, AskableReport, AskedReport, Became, Counted,
    LeftAfter, MemberAsking, Stands, TitleKind, TitleUnit,
};

use super::{askable, asked, kinds};

/// What a member's period allows of one kind: `limit` of it, `used` spent.
fn counted(limit: Option<u32>, used: u32) -> Counted {
    Counted {
        limit,
        used,
        remaining: limit.map(|limit| limit - used),
        period: limit.map(|_| "a month".to_owned()),
    }
}

/// A page holding one title of each standing, under a limit on films.
fn a_page(next: Option<&str>, titles: Vec<Askable>) -> AskableReport {
    AskableReport {
        member: "ana".to_owned(),
        term: "harbour".to_owned(),
        kinds: vec![TitleKind::Tv, TitleKind::Movies],
        titles,
        next: next.map(str::to_owned),
        asking: MemberAsking {
            policy: Policy::EverythingWaits,
            standing: Standing::NearQuota,
            films: counted(Some(2), 1),
            television: counted(None, 0),
            frees_up: None,
        },
        approval: "Nothing arrives until whoever runs the house has said yes to it.".to_owned(),
    }
}

/// A title of `kind` standing as `stands`.
fn a_title(id: &str, kind: TitleKind, year: Option<u16>, stands: Stands) -> Askable {
    Askable {
        id: id.to_owned(),
        kind,
        unit: None,
        title: format!("Harbour {id}"),
        by: None,
        year,
        cover: None,
        stands,
    }
}

#[test]
fn a_page_lists_each_title_with_where_it_stands_and_where_the_next_begins() {
    let text = askable(&a_page(
        Some("next-place"),
        vec![
            a_title("1", TitleKind::Movies, Some(1999), Stands::Here),
            a_title("2", TitleKind::Tv, None, Stands::PartlyHere),
            a_title("3", TitleKind::Movies, Some(2003), Stands::OnItsWay),
            a_title("4", TitleKind::Tv, Some(2011), Stands::Askable),
        ],
    ))
    .text();
    assert!(
        text.contains("ana — \"harbour\", among every series and every film"),
        "{text}"
    );
    assert!(
        text.contains("Harbour 1 (1999) · film · here · 1"),
        "{text}"
    );
    assert!(
        text.contains("Harbour 2 · series · partly here · 2"),
        "{text}"
    );
    assert!(
        text.contains("Harbour 3 (2003) · film · on its way · 3"),
        "{text}"
    );
    assert!(
        text.contains("Harbour 4 (2011) · series · may be asked for · 4"),
        "{text}"
    );
    assert!(
        text.contains("Nothing arrives until whoever runs the house has said yes to it."),
        "{text}"
    );
    assert!(
        text.contains("Their allowance: 1 of 2 a month used"),
        "{text}"
    );
    assert!(text.contains("More: --after next-place"), "{text}");
}

#[test]
fn a_page_with_nothing_on_it_says_so_and_offers_no_more() {
    let text = askable(&a_page(None, Vec::new())).text();
    assert!(
        text.contains("Nothing on this page they may ask for."),
        "{text}"
    );
    assert!(!text.contains("More:"), "{text}");
}

#[test]
fn a_page_for_a_member_nothing_limits_names_no_allowance() {
    let mut page = a_page(None, Vec::new());
    page.asking.films = counted(None, 0);
    let text = askable(&page).text();
    assert!(!text.contains("Their allowance"), "{text}");
}

#[test]
fn an_ask_is_said_in_the_households_own_sentence() {
    let text = asked(&AskedReport {
        member: "ana".to_owned(),
        title: "1399".to_owned(),
        kind: TitleKind::Tv,
        unit: None,
        seasons: vec![3],
        request: None,
        outcome: Became::Waiting,
        leaves: LeftAfter::default(),
        released: true,
        said: "Asked. Whoever runs the house will say yes or no.".to_owned(),
        rehearsed: false,
    })
    .text();
    assert_eq!(
        text.trim(),
        "ana — Asked. Whoever runs the house will say yes or no."
    );
}

#[test]
fn an_album_is_listed_by_its_artist_and_its_ask_by_the_reference_it_is_kept_under() {
    let text = askable(&a_page(
        None,
        vec![Askable {
            unit: Some(TitleUnit::Album),
            by: Some("Miles Davis".to_owned()),
            ..a_title("mbid-1", TitleKind::Music, Some(1959), Stands::Askable)
        }],
    ))
    .text();
    assert!(
        text.contains("Harbour mbid-1 by Miles Davis (1959) · album · may be asked for · mbid-1"),
        "{text}"
    );

    let said = asked(&AskedReport {
        member: "ana".to_owned(),
        title: "mbid-1".to_owned(),
        kind: TitleKind::Music,
        unit: Some(TitleUnit::Album),
        seasons: Vec::new(),
        request: Some("music-1".to_owned()),
        outcome: Became::Waiting,
        leaves: LeftAfter::default(),
        released: true,
        said: "Asked. Whoever runs the house will say yes or no.".to_owned(),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("On the household list as music-1."), "{said}");
}

#[test]
fn a_search_and_an_ask_are_rendered_by_their_own_shapes() {
    let page = a_page(None, Vec::new());
    let report = AskedReport {
        member: "ana".to_owned(),
        title: "603".to_owned(),
        kind: TitleKind::Movies,
        unit: None,
        seasons: Vec::new(),
        request: None,
        outcome: Became::OnItsWay,
        leaves: LeftAfter::default(),
        released: true,
        said: "Somebody already asked for this, so it's on its way.".to_owned(),
        rehearsed: false,
    };

    assert_eq!(
        crate::render::shaped(&Outcome::Askable(page.clone())).text(),
        askable(&page).text()
    );
    assert_eq!(
        crate::render::shaped(&Outcome::Asked(report.clone())).text(),
        asked(&report).text()
    );
}

#[test]
fn the_kinds_a_member_may_ask_for_are_a_kind_a_line_with_its_units() {
    let report = AskableKindsReport {
        member: "ana".to_owned(),
        kinds: vec![
            AskableKind {
                kind: TitleKind::Tv,
                units: Vec::new(),
            },
            AskableKind {
                kind: TitleKind::Movies,
                units: Vec::new(),
            },
            AskableKind {
                kind: TitleKind::Music,
                units: vec![TitleUnit::Album, TitleUnit::Artist],
            },
            AskableKind {
                kind: TitleKind::Books,
                units: vec![TitleUnit::Book, TitleUnit::Series],
            },
        ],
    };

    assert_eq!(
        kinds(&report).text(),
        "ana may ask for\n  series\n  films\n  music: album, artist\n  books: book, book series"
    );
    let nothing = AskableKindsReport {
        member: "ana".to_owned(),
        kinds: Vec::new(),
    };
    assert_eq!(
        crate::render::shaped(&Outcome::AskableKinds(nothing)).text(),
        "ana may ask for\n  Nothing right now."
    );
}

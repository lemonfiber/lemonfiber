use super::{
    Claimed, File, Health, Invitation, Key, Lapse, Lapses, Left, Outcome, Refusal, Refusals, Table,
    TokenHash, FORMAT,
};

fn invitation(token: &str, lapses: u64) -> Invitation {
    Invitation {
        token: TokenHash::of(token),
        account: "8c7a".to_owned(),
        name: "Ana".to_owned(),
        issued: 1_000,
        lapses,
    }
}

#[test]
fn a_token_is_held_as_its_sha256_in_lowercase_hex() {
    assert_eq!(
        TokenHash::of("abc").as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn the_table_finds_an_invitation_by_its_token_and_nothing_else() {
    let table = Table::of(
        Claimed::HasPassword,
        vec![invitation("one", 2_000), invitation("two", 2_000)],
    );

    assert_eq!(
        table.find(&TokenHash::of("two")).map(|one| one.lapses),
        Some(2_000)
    );
    assert!(table.find(&TokenHash::of("three")).is_none());
}

#[test]
fn an_invitation_is_open_until_it_lapses() {
    let open = invitation("one", 2_000);

    assert!(open.open_at(1_999));
    assert!(!open.open_at(2_000));
}

#[test]
fn a_table_reads_back_as_it_was_written() {
    let table = Table::of(Claimed::HasPassword, vec![invitation("one", 2_000)]);

    assert_eq!(Table::read(&table.written()), Ok(table));
}

#[test]
fn a_table_naming_no_strategy_is_one_under_which_nothing_is_removed() {
    let unsaid = format!("{{\"format\": {FORMAT}, \"invitations\": []}}");

    assert_eq!(
        Table::read(&unsaid).map(|table| table.claimed),
        Ok(Claimed::Unknown)
    );
}

#[test]
fn the_written_table_holds_no_token() {
    let written = Table::of(
        Claimed::HasPassword,
        vec![invitation("a-token-nobody-should-see", 2_000)],
    )
    .written();

    assert!(!written.contains("a-token-nobody-should-see"));
    assert!(written.ends_with('\n'));
}

#[test]
fn a_table_in_another_format_is_refused_rather_than_read() {
    let other = format!("{{\"format\": {}, \"invitations\": []}}", FORMAT + 1);

    let refused = Table::read(&other).err();

    assert_eq!(
        refused.as_ref().map(|one| one.file),
        Some(File::Table.name())
    );
    assert!(refused.is_some_and(|one| one.why.contains("format 2")));
}

#[test]
fn text_that_is_not_a_table_is_refused_naming_the_file() {
    let refused = Table::read("not json").err();

    assert_eq!(
        refused.map(|one| one.to_string().starts_with("invitations.json")),
        Some(true)
    );
}

#[test]
fn a_second_refusal_of_the_same_invitation_records_nothing() {
    let first = Refusal {
        token: TokenHash::of("one"),
        account: "8c7a".to_owned(),
        at: 1_500,
    };
    let again = Refusal {
        at: 1_600,
        ..first.clone()
    };

    let refusals = Refusals::default().with(first).with(again);

    assert_eq!(refusals.refusals.len(), 1);
    assert_eq!(
        refusals.of(&TokenHash::of("one")).map(|one| one.at),
        Some(1_500)
    );
}

#[test]
fn refusals_read_back_as_they_were_written() {
    let refusals = Refusals::default().with(Refusal {
        token: TokenHash::of("one"),
        account: "8c7a".to_owned(),
        at: 1_500,
    });

    assert_eq!(Refusals::read(&refusals.written()), Ok(refusals));
}

#[test]
fn refusals_in_another_format_are_refused() {
    let refused = Refusals::read("{\"format\": 0, \"refusals\": []}").err();

    assert_eq!(refused.map(|one| one.file), Some(File::Refusals.name()));
}

#[test]
fn a_key_is_read_without_its_line_ending_and_written_with_one() {
    let key = Key::read("  0123abcd\n");

    assert_eq!(key.as_ref().map(Key::reveal), Ok("0123abcd"));
    assert_eq!(key.map(|one| one.written()), Ok("0123abcd\n".to_owned()));
}

#[test]
fn an_empty_key_file_or_one_holding_two_words_is_refused() {
    assert!(Key::read("\n").is_err());
    assert!(Key::read("one two").is_err());
}

#[test]
fn a_key_is_withheld_from_debug() {
    let key = Key::read("0123abcd").map(|one| format!("{one:?}"));

    assert_eq!(key, Ok("Key(withheld)".to_owned()));
}

#[test]
fn every_file_has_its_own_name() {
    assert_eq!(File::Table.name(), "invitations.json");
    assert_eq!(File::Key.name(), "jellyfin.key");
    assert_eq!(File::Refusals.name(), "refusals.json");
    assert_eq!(File::Lapses.name(), "lapses.json");
}

#[test]
fn a_key_is_named_by_its_sha256_and_not_by_itself() {
    let fingerprint = Key::read("abc").map(|one| one.fingerprint());

    assert_eq!(
        fingerprint.as_deref(),
        Ok("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
}

#[test]
fn health_says_which_key_the_service_holds_and_reads_back() {
    let held = Key::read("one").ok();
    let other = Key::read("two").ok();
    let health = Health::holding(held.as_ref());

    assert!(held.as_ref().is_some_and(|key| health.holds(key)));
    assert!(other.as_ref().is_some_and(|key| !health.holds(key)));
    assert!(held
        .as_ref()
        .is_some_and(|key| !Health::holding(None).holds(key)));
    let said = serde_json::to_string(&health).unwrap_or_default();
    assert_eq!(serde_json::from_str::<Health>(&said).ok(), Some(health));
    assert_eq!(
        serde_json::to_string(&Health::holding(None))
            .ok()
            .as_deref(),
        Some(r#"{"key":null}"#)
    );
}

fn lapse(token: &str, at: u64, outcome: Outcome) -> Lapse {
    Lapse {
        token: TokenHash::of(token),
        account: "8c7a".to_owned(),
        name: "Ana".to_owned(),
        issued: 1_000,
        at,
        outcome,
    }
}

#[test]
fn a_strategy_this_build_does_not_know_is_read_as_unknown_rather_than_refused() {
    let later = format!(
        "{{\"format\": {FORMAT}, \"claimed\": \"asks-a-crystal-ball\", \"invitations\": []}}"
    );

    assert_eq!(
        Table::read(&later).map(|table| table.claimed),
        Ok(Claimed::Unknown)
    );
}

#[test]
fn the_strategy_a_table_names_reads_back_as_it_was_written() {
    let table = Table {
        claimed: Claimed::OwnWrites,
        ..Table::of(Claimed::HasPassword, vec![invitation("one", 2_000)])
    };

    assert!(table.written().contains("\"own-writes\""));
    assert_eq!(Table::read(&table.written()), Ok(table));
}

#[test]
fn an_invitation_is_taken_back_once() {
    let lapses = Lapses::default()
        .with(lapse("one", 2_100, Outcome::Removed))
        .with(lapse("one", 2_200, Outcome::SwitchedOff));

    assert_eq!(lapses.lapses.len(), 1);
    assert_eq!(
        lapses.of(&TokenHash::of("one")).map(|one| one.outcome),
        Some(Outcome::Removed)
    );
}

#[test]
fn the_latest_lapse_of_an_account_is_the_one_last_recorded_against_it() {
    let lapses = Lapses::default()
        .with(lapse("first", 2_100, Outcome::SwitchedOff))
        .with(lapse("second", 4_100, Outcome::Left(Left::Claimed)));

    assert_eq!(lapses.latest_for("8c7a").map(|one| one.at), Some(4_100));
    assert!(lapses.latest_for("somebody-else").is_none());
}

#[test]
fn lapses_read_back_as_they_were_written_and_say_why_one_was_left() {
    let lapses = Lapses::default()
        .with(lapse("one", 2_100, Outcome::Removed))
        .with(lapse("two", 2_100, Outcome::Left(Left::Reoffered)));

    let written = lapses.written();
    assert!(written.contains("\"removed\""));
    assert!(written.contains("\"reoffered\""));
    assert_eq!(Lapses::read(&written), Ok(lapses));
}

#[test]
fn lapses_in_another_format_are_refused() {
    let refused = Lapses::read("{\"format\": 0, \"lapses\": []}").err();

    assert_eq!(refused.map(|one| one.file), Some(File::Lapses.name()));
}

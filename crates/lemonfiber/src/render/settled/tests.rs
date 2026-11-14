use super::settled;
use lemonfiber_core::config::Protocols;
use lemonfiber_core::model::{ServiceUser, SetUp, SettledReport, UserFrom};

fn a_report(setup: SetUp, from: UserFrom, protocols: Protocols) -> SettledReport {
    SettledReport {
        setup,
        data_root: Some("/srv/media".to_owned()),
        protocols,
        service_user: ServiceUser {
            uid: 1234,
            gid: 4321,
            from,
        },
    }
}

fn said(report: &SettledReport) -> String {
    settled(report).text()
}

#[test]
fn each_standing_of_setup_is_said_in_its_own_words() {
    let none = said(&a_report(SetUp::None, UserFrom::Set, Protocols::none()));
    let part = said(&a_report(SetUp::PartWay, UserFrom::Set, Protocols::none()));
    let done = said(&a_report(SetUp::Done, UserFrom::Set, Protocols::none()));

    assert!(none.contains("not set up"), "{none}");
    assert!(part.contains("part-way"), "{part}");
    assert!(done.contains("This stack is set up."), "{done}");
}

#[test]
fn the_data_root_the_protocols_and_the_service_user_are_said() {
    let text = said(&a_report(SetUp::Done, UserFrom::Set, Protocols::both()));

    assert!(text.contains("/srv/media"), "{text}");
    assert!(text.contains("Usenet and torrents"), "{text}");
    assert!(text.contains("1234:4321"), "{text}");
    assert!(text.contains("PUID and PGID set them"), "{text}");
}

#[test]
fn where_the_service_user_comes_from_is_said_for_each_source() {
    let part = said(&a_report(SetUp::Done, UserFrom::PartSet, Protocols::none()));
    let owner = said(&a_report(
        SetUp::Done,
        UserFrom::StackOwner,
        Protocols::none(),
    ));

    assert!(part.contains("one of PUID and PGID set"), "{part}");
    assert!(owner.contains("neither PUID nor PGID is set"), "{owner}");
    assert!(owner.contains("none configured"), "{owner}");
}

#[test]
fn one_protocol_is_named_alone_and_a_data_root_not_chosen_is_said() {
    let mut usenet = a_report(
        SetUp::None,
        UserFrom::StackOwner,
        Protocols {
            usenet: true,
            torrent: false,
        },
    );
    usenet.data_root = None;
    let torrent = a_report(
        SetUp::None,
        UserFrom::StackOwner,
        Protocols {
            usenet: false,
            torrent: true,
        },
    );

    let text = said(&usenet);
    assert!(text.contains("Downloading   Usenet"), "{text}");
    assert!(text.contains("not chosen"), "{text}");
    assert!(said(&torrent).contains("torrents"));
}

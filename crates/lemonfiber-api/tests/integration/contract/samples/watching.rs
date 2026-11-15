//! The live-TV samples: the channels a member is offered, their guide, a stream opened on
//! a pass, and a title playing in a browser.

/// One channel with the programme on it now, with every field a channel can carry.
pub(super) fn the_channels() -> lemonfiber_core::model::ChannelsReport {
    use lemonfiber_core::ports::service::{Channel, Programme};
    lemonfiber_core::model::ChannelsReport {
        channels: vec![Channel {
            id: "news".to_owned(),
            name: "News".to_owned(),
            number: Some("101".to_owned()),
            now: Some(Programme {
                title: "The evening news".to_owned(),
                certificate: Some("PG".to_owned()),
                ends: Some(lemonfiber_core::instant::Instant::from_seconds(
                    1_794_596_400,
                )),
            }),
        }],
        ..lemonfiber_core::model::ChannelsReport::default()
    }
}

/// One channel in a guide with a programme on it, with every field a guide can carry.
pub(super) fn the_guide() -> lemonfiber_core::model::GuideReport {
    use lemonfiber_core::instant::Instant;
    use lemonfiber_core::model::{GuideReport, Guided};
    use lemonfiber_core::ports::service::Airing;
    GuideReport {
        channels: vec![Guided {
            id: "news".to_owned(),
            name: "News".to_owned(),
            number: Some("101".to_owned()),
            programmes: vec![Airing {
                title: "The evening news".to_owned(),
                certificate: Some("PG".to_owned()),
                starts: Instant::from_seconds(1_794_594_600),
                ends: Instant::from_seconds(1_794_596_400),
            }],
            guide_until: Some(Instant::from_seconds(1_794_596_400)),
        }],
        ..GuideReport::default()
    }
}

/// A live stream opened at the door, on a pass.
pub(super) fn a_tune() -> lemonfiber_core::model::TunedReport {
    lemonfiber_core::model::TunedReport {
        stream: Some("ls1".to_owned()),
        direct: Some("https://house.local:8920/live/ls1/stream.ts?lfpass=a.1.b".to_owned()),
        fingerprint: Some("ab".repeat(32)),
        lapses: Some(1_800_000_300),
        ..lemonfiber_core::model::TunedReport::default()
    }
}

/// A title playing in a browser from the front door, on a pass.
pub(super) fn a_play() -> lemonfiber_core::model::PlayReport {
    lemonfiber_core::model::PlayReport {
        stream_from: Some("https://watch.example.org/master.m3u8?lfpass=a.1.b".to_owned()),
        lapses: Some(1_800_000_300),
        ..lemonfiber_core::model::PlayReport::default()
    }
}

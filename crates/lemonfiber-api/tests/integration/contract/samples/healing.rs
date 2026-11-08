//! The queue's sample: what healing found and what its newest run did.

/// A queue with healing on, one download stalled and one orphaned, a client that names no
/// download by its id, and a run that dropped one and could not drop another.
pub(super) fn a_queue() -> lemonfiber_core::model::QueueHealth {
    use lemonfiber_core::model::{
        Act, ActOutcome, ActTaken, HealRun, Healed, HealingSwitch, QueueHealth, Unreached, Wedge,
        Wedged,
    };
    let done = |act| ActTaken {
        act,
        outcome: ActOutcome::Done,
        instead: None,
        why: None,
    };
    QueueHealth {
        healing: HealingSwitch::On,
        strikes_needed: 3,
        grace_s: 1_800,
        items: vec![
            Wedged {
                title: Some("The Office".to_owned()),
                service: "sonarr".to_owned(),
                release: "The.Office.S01E01.1080p".to_owned(),
                class: Wedge::Stalled,
                why: "it is connected with nothing moving and no source to move it".to_owned(),
                strikes: 2,
                added_at: Some(1_762_400_000),
                in_grace_until: None,
                next: Some(vec![Act::Remove, Act::Blocklist, Act::Search]),
            },
            Wedged {
                title: None,
                service: "qbittorrent".to_owned(),
                release: "Leftover.2019.mkv".to_owned(),
                class: Wedge::Orphaned,
                why: "its client holds it under tv, and no curator's queue does".to_owned(),
                strikes: 1,
                added_at: Some(1_762_500_000),
                in_grace_until: Some(1_762_501_800),
                next: Some(Vec::new()),
            },
        ],
        unreachable: vec![Unreached {
            service: "sabnzbd".to_owned(),
            why: "it names no download by its own id, so its downloads are judged from their \
                  curators' view alone and none is found orphaned"
                .to_owned(),
        }],
        mismatch: None,
        incomplete: false,
        runs: vec![HealRun {
            at: 1_762_499_400,
            rehearsed: false,
            items: vec![
                Healed {
                    title: Some("Dune".to_owned()),
                    service: "radarr".to_owned(),
                    release: "Dune.2021.2160p".to_owned(),
                    why: "the torrent has not resolved what it is".to_owned(),
                    acts: vec![done(Act::Remove), done(Act::Blocklist), done(Act::Search)],
                },
                Healed {
                    title: Some("Doctor Who".to_owned()),
                    service: "medusa".to_owned(),
                    release: "Doctor.Who.S01E01".to_owned(),
                    why: "its curator gave up on it".to_owned(),
                    acts: vec![
                        ActTaken {
                            act: Act::Remove,
                            outcome: ActOutcome::FellBack,
                            instead: Some(Act::Search),
                            why: Some("medusa cannot drop a download".to_owned()),
                        },
                        ActTaken {
                            act: Act::Blocklist,
                            outcome: ActOutcome::Refused,
                            instead: None,
                            why: Some("medusa cannot drop a download".to_owned()),
                        },
                        done(Act::Search),
                    ],
                },
            ],
        }],
        classified_at: Some(1_762_500_300),
        rehearsed: false,
    }
}

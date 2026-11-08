use lemonfiber_core::model::{
    Act, ActOutcome, ActTaken, HealRun, Healed, HealingSwitch, QueueHealth, Unreached, Wedge,
    Wedged,
};

use super::queue;

/// A download wedged for `class`, at `service`.
fn wedged(class: Wedge, title: Option<&str>, next: Option<Vec<Act>>) -> Wedged {
    Wedged {
        title: title.map(str::to_owned),
        service: "sonarr".to_owned(),
        release: "The.Office.S01E01".to_owned(),
        class,
        why: "it is connected with nothing moving".to_owned(),
        strikes: 2,
        added_at: Some(1_759_400_000),
        in_grace_until: None,
        next,
    }
}

/// `act` as it came out.
fn act(act: Act, outcome: ActOutcome, instead: Option<Act>, why: Option<&str>) -> ActTaken {
    ActTaken {
        act,
        outcome,
        instead,
        why: why.map(str::to_owned),
    }
}

#[test]
fn what_is_wedged_why_and_what_each_run_did_are_said_act_by_act() {
    let mut young = wedged(Wedge::StuckOnMetadata, None, Some(Vec::new()));
    young.in_grace_until = Some(1_759_401_800);
    let said = queue(&QueueHealth {
        healing: HealingSwitch::On,
        strikes_needed: 3,
        grace_s: 1_800,
        items: vec![
            wedged(
                Wedge::Stalled,
                Some("The Office"),
                Some(vec![Act::Remove, Act::Blocklist, Act::Search]),
            ),
            young,
            wedged(Wedge::Slow, Some("Slow"), None),
            wedged(Wedge::Failed, Some("Failed"), None),
            wedged(Wedge::Orphaned, Some("Orphan"), None),
        ],
        unreachable: vec![Unreached {
            service: "qbittorrent".to_owned(),
            why: "it does not answer".to_owned(),
        }],
        mismatch: Some("qbittorrent files tv under /elsewhere".to_owned()),
        incomplete: true,
        runs: vec![
            HealRun {
                at: 1_759_400_000,
                rehearsed: false,
                items: vec![Healed {
                    title: Some("The Office".to_owned()),
                    service: "sonarr".to_owned(),
                    release: "The.Office.S01E01".to_owned(),
                    why: "it stalled".to_owned(),
                    acts: vec![
                        act(
                            Act::Remove,
                            ActOutcome::FellBack,
                            Some(Act::Search),
                            Some("unsupported"),
                        ),
                        act(
                            Act::Blocklist,
                            ActOutcome::Refused,
                            None,
                            Some("unsupported"),
                        ),
                        act(Act::Search, ActOutcome::Done, None, None),
                    ],
                }],
            },
            HealRun {
                at: 1_759_300_000,
                rehearsed: true,
                items: vec![Healed {
                    title: None,
                    service: "radarr".to_owned(),
                    release: "Film.2020".to_owned(),
                    why: "it failed".to_owned(),
                    acts: vec![
                        act(Act::Remove, ActOutcome::Would, None, Some("rehearsed")),
                        act(Act::Blocklist, ActOutcome::FellBack, None, None),
                    ],
                }],
            },
        ],
        classified_at: Some(1_759_400_000),
        rehearsed: true,
    })
    .text();

    for line in [
        "Healing is on: a download flagged for one reason on 3 passes in a row, past its first 30 minutes",
        "This run only said what it would do.",
        "Last looked at 2025-10-02T10:13:20 UTC.",
        "Something could not be read, so this list may be short.",
        "Nothing is acted on where qbittorrent files tv under /elsewhere.",
        "  qbittorrent: it does not answer",
        "  The Office at sonarr: stalled, it is connected with nothing moving; strike 2 of 3",
        "    next run: drop, blocklist, search again",
        "  The.Office.S01E01 at sonarr: stuck on metadata",
        "    left alone until 2025-10-02T10:43:20 UTC",
        "    next run: nothing, as no curator holds it by a queue entry",
        "  Slow at sonarr: slow",
        "  Failed at sonarr: failed",
        "  Orphan at sonarr: orphaned",
        "  2025-10-02T10:13:20 UTC\n",
        "    The Office at sonarr, because it stalled",
        "      drop: not done, search again instead, unsupported; blocklist: not done, unsupported; search again: done",
        ", rehearsed",
        "    Film.2020 at radarr, because it failed",
        "      drop: would; blocklist: not done",
    ] {
        assert!(said.contains(line), "{line}\n{said}");
    }
}

#[test]
fn a_queue_off_or_never_looked_at_says_so_and_a_healthy_one_says_nothing_is_wedged() {
    let never = queue(&QueueHealth::default()).text();
    assert!(
        never.contains("Healing is off: the queue is classified, and nothing is acted on."),
        "{never}"
    );
    assert!(
        never.contains("The queue has not been looked at yet."),
        "{never}"
    );
    assert!(
        !never.contains("Nothing in the queue is wedged."),
        "{never}"
    );

    let healthy = queue(&QueueHealth {
        classified_at: Some(1_759_400_000),
        ..QueueHealth::default()
    })
    .text();
    assert!(
        healthy.contains("Nothing in the queue is wedged."),
        "{healthy}"
    );
    assert!(!healthy.contains("rehearsed"), "{healthy}");
}

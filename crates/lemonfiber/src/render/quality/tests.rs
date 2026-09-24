use super::*;
use crate::render::fixtures::*;
use lemonfiber_core::model::{
    Disposition, MusicReport, QualityReport, Triggered, UpgradeMedia, UpgradeReport,
};

#[test]
fn a_preset_choice_warns_only_where_this_machine_would_transcode() {
    assert!(preset_choice(&preset(true))
        .text()
        .contains("transcode this in software"));
    assert!(!preset_choice(&preset(false))
        .text()
        .contains("transcode this in software"));
}

#[test]
fn every_quality_disposition_says_what_became_of_the_choice() {
    for (disposition, customised, expected) in [
        (Disposition::Recorded, false, "Saved."),
        (Disposition::Recorded, true, "quality reapply"),
        (Disposition::Rehearsed, false, "Would save."),
        (Disposition::Held, false, "Not saved"),
        (Disposition::Reapplied, true, "overwriting your customised"),
        (Disposition::Reapplied, false, "already in step"),
        (
            Disposition::WouldReapply,
            true,
            "Would reapply the preset, overwriting",
        ),
        (Disposition::WouldReapply, false, "already in step"),
        (Disposition::Shown, true, "no longer authoritative"),
    ] {
        let report = QualityReport {
            choices: vec![preset(false)],
            music: Some(music_pick()),
            customised,
            overwritten: None,
            disposition,
        };
        let text = quality(&report).text();
        assert!(text.contains(expected), "{disposition:?}: {text}");
    }
    // Shown with an untouched config says nothing extra.
    let plain = QualityReport {
        choices: vec![preset(false)],
        music: None,
        customised: false,
        overwritten: None,
        disposition: Disposition::Shown,
    };
    assert!(!quality(&plain).text().contains("authoritative"));
}

/// Consent to an overwrite is consent to something the operator was shown.
#[test]
fn a_reapply_that_replaced_an_edit_shows_which_lines_went() {
    for disposition in [Disposition::Reapplied, Disposition::WouldReapply] {
        let report = QualityReport {
            choices: vec![preset(false)],
            music: None,
            customised: true,
            overwritten: Some(lemonfiber_core::model::StackEdit {
                path: "config/recyclarr/recyclarr.yml".to_owned(),
                diff: "- # mine\n+ include:\n".to_owned(),
            }),
            disposition,
        };
        let text = quality(&report).text();
        assert!(text.contains("What went, in config/recyclarr"), "{text}");
        assert!(text.contains("- # mine"), "{text}");
        assert!(text.contains("+ include:"), "{text}");
    }
}

/// And nothing is offered where there is nothing to show, so the sentence and the
/// lines cannot disagree.
#[test]
fn a_reapply_with_nothing_to_replace_shows_no_lines() {
    let report = QualityReport {
        choices: vec![preset(false)],
        music: None,
        customised: false,
        overwritten: None,
        disposition: Disposition::Reapplied,
    };
    assert!(!quality(&report).text().contains("What went"));
}

#[test]
fn the_music_choice_reports_what_became_of_applying_it() {
    for (outcome, expected) in [
        (None, "Applied to the music service."),
        (Some(Triggered::Started), "Applied to the music service."),
        (Some(Triggered::NotStarted), "not up yet"),
        (
            Some(Triggered::Failed {
                detail: "refused".to_owned(),
            }),
            "refused the change: refused",
        ),
    ] {
        let report = MusicReport {
            choice: music_pick(),
            disposition: Disposition::Recorded,
            outcome,
        };
        assert!(music(&report).text().contains(expected));
    }
    // A rehearsal stops at "would save" and never claims it applied anything.
    let rehearsed = MusicReport {
        choice: music_pick(),
        disposition: Disposition::Rehearsed,
        outcome: None,
    };
    let text = music(&rehearsed).text();
    assert!(text.contains("Would save."));
    assert!(!text.contains("Applied"));
}

#[test]
fn an_upgrade_states_its_cost_before_it_is_confirmed() {
    let media = vec![UpgradeMedia {
        media_type: "tv".to_owned(),
        preset: "Balanced".to_owned(),
        size_per_hour: "3 GB".to_owned(),
        outcome: None,
    }];
    let unconfirmed = UpgradeReport {
        confirmed: false,
        media: media.clone(),
    };
    let text = upgrade(&unconfirmed).text();
    assert!(text.contains("bandwidth-expensive"));
    assert!(text.contains("Nothing has been changed."));
    // Nothing to upgrade is said plainly rather than shown as an empty list.
    let nothing = UpgradeReport {
        confirmed: false,
        media: Vec::new(),
    };
    assert!(upgrade(&nothing).text().contains("nothing to upgrade"));
}

#[test]
fn a_confirmed_upgrade_reports_each_services_answer() {
    for (outcome, expected) in [
        (Some(Triggered::Started), "re-search started"),
        (Some(Triggered::NotStarted), "not started"),
        (
            Some(Triggered::Failed {
                detail: "boom".to_owned(),
            }),
            "✗ boom",
        ),
    ] {
        let report = UpgradeReport {
            confirmed: true,
            media: vec![UpgradeMedia {
                media_type: "tv".to_owned(),
                preset: "Balanced".to_owned(),
                size_per_hour: "3 GB".to_owned(),
                outcome,
            }],
        };
        assert!(upgrade(&report).text().contains(expected));
    }
}

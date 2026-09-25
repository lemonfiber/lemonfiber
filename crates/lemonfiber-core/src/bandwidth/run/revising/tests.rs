use super::{defaulted_upload, revised, Asked};
use crate::bandwidth::capacity::Source;
use crate::bandwidth::limit::UPLOAD_SHARE;
use crate::bandwidth::{Cap, Capacity, Declared, Limit, WhenExceeded, UNREADABLE};

/// A moment every case here reads against.
const NOW: u64 = 1_790_812_800;

/// A request naming one thing.
fn asking(field: impl FnOnce(&mut Asked)) -> Asked {
    let mut asked = Asked::default();
    field(&mut asked);
    asked
}

/// What a request revises a fresh declaration into.
fn from_nothing(asked: &Asked) -> Result<Declared, String> {
    revised(NOW, Declared::default(), asked).map_err(|problem| problem.code.as_str().to_owned())
}

#[test]
fn a_download_limit_can_be_a_share_or_a_figure() {
    assert_eq!(
        from_nothing(&asking(|asked| asked.down = Some("50%".to_owned())))
            .ok()
            .and_then(|declared| declared.down),
        Some(Limit::Share(50))
    );
    assert_eq!(
        from_nothing(&asking(|asked| asked.down = Some("2MiB".to_owned())))
            .ok()
            .and_then(|declared| declared.down),
        Some(Limit::Absolute(2 * 1024 * 1024))
    );
}

#[test]
fn an_unstated_upload_limit_is_always_more_careful_than_the_download_one() {
    // The requirement, held as a rule rather than as two numbers somebody
    // keeps in order by hand. Whatever the download asks for, the upload asks
    // for no more.
    for share in 1..=100_u8 {
        let defaulted = defaulted_upload(
            &asking(|asked| asked.down = Some(format!("{share}%"))),
            Some(Limit::Share(share)),
        );
        assert_eq!(defaulted, Some(Limit::Share(share.min(UPLOAD_SHARE))));
        assert!(
            defaulted.is_some_and(|up| matches!(up, Limit::Share(up) if up <= share)),
            "{share}%"
        );
    }
}

/// A figure asked for in bytes defaults an upload in bytes, not a share.
///
/// The refusal for a share of an unmeasured line offers exactly this as the way
/// out — *give a figure instead of a share*, `--down 2MiB`. Defaulting that to a
/// share of the same unmeasured line refused the operator for an upload they had
/// not mentioned, and made the remedy unfollowable: doing what it said reproduced
/// the error it said it would avoid.
#[test]
fn an_absolute_download_limit_leaves_the_upload_careful_in_the_same_terms() {
    assert_eq!(
        defaulted_upload(
            &asking(|asked| asked.down = Some("2MiB".to_owned())),
            Some(Limit::Absolute(2 * 1024 * 1024))
        ),
        Some(Limit::Absolute(2 * 1024 * 1024 / 4))
    );
    // Never nothing, however small the figure it is a quarter of.
    assert_eq!(
        defaulted_upload(
            &asking(|asked| asked.down = Some("1".to_owned())),
            Some(Limit::Absolute(1))
        ),
        Some(Limit::Absolute(1))
    );
}

#[test]
fn an_upload_limit_that_was_asked_for_is_never_defaulted_over() {
    let asked = Asked {
        down: Some("50%".to_owned()),
        up: Some("unlimited".to_owned()),
        ..Asked::default()
    };
    assert_eq!(defaulted_upload(&asked, Some(Limit::Share(50))), None);
    assert_eq!(
        from_nothing(&asked).ok().and_then(|declared| declared.up),
        Some(Limit::Unlimited)
    );
}

#[test]
fn lifting_the_download_limit_does_not_invent_an_upload_one() {
    assert_eq!(
        defaulted_upload(
            &asking(|asked| asked.down = Some("unlimited".to_owned())),
            Some(Limit::Unlimited)
        ),
        None
    );
}

#[test]
fn a_cap_declared_with_nothing_to_do_at_it_is_refused_rather_than_defaulted() {
    // The whole of the requirement is that the choice is made in advance. A
    // default chosen here is a choice nobody made, arriving at two in the
    // morning on a stack nobody is watching.
    let refused = from_nothing(&asking(|asked| asked.cap = Some("1TiB".to_owned())));
    assert_eq!(refused, Err(UNREADABLE.as_str().to_owned()));
}

#[test]
fn a_cap_that_already_has_a_choice_keeps_it_when_the_figure_changes() {
    let held = Declared {
        cap: Some(Cap {
            monthly: 1,
            exceeded: WhenExceeded::Throttle,
        }),
        ..Declared::default()
    };
    let changed = revised(
        NOW,
        held,
        &asking(|asked| asked.cap = Some("1TiB".to_owned())),
    );
    assert_eq!(
        changed.ok().and_then(|declared| declared.cap),
        Some(Cap {
            monthly: 1024_u64.pow(4),
            exceeded: WhenExceeded::Throttle
        })
    );
}

#[test]
fn what_to_do_at_a_cap_nobody_declared_cannot_be_answered_as_it_stands() {
    let refused = from_nothing(&asking(|asked| asked.exceeded = Some("pause".to_owned())));
    assert_eq!(refused, Err(UNREADABLE.as_str().to_owned()));
}

#[test]
fn what_to_do_at_the_cap_can_be_changed_without_restating_the_figure() {
    let held = Declared {
        cap: Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Continue,
        }),
        ..Declared::default()
    };
    let changed = revised(
        NOW,
        held,
        &asking(|asked| asked.exceeded = Some("pause".to_owned())),
    );
    assert_eq!(
        changed.ok().and_then(|declared| declared.cap),
        Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Pause
        })
    );
}

#[test]
fn a_word_this_does_not_know_is_refused_wherever_it_appears() {
    for asked in [
        asking(|asked| asked.down = Some("half".to_owned())),
        asking(|asked| asked.up = Some("loads".to_owned())),
        asking(|asked| asked.active = Some("evenings".to_owned())),
        asking(|asked| asked.line = Some("fast".to_owned())),
        asking(|asked| asked.line = Some("60MiB".to_owned())),
        asking(|asked| asked.line = Some("60MiB/0".to_owned())),
        asking(|asked| asked.cap = Some("lots".to_owned())),
    ] {
        assert_eq!(
            from_nothing(&asked),
            Err(UNREADABLE.as_str().to_owned()),
            "{asked:?}"
        );
    }
}

#[test]
fn a_setting_can_be_taken_away_again_by_saying_so() {
    // One of the three words each, so none of them is a word only the reader
    // of this function knows about. A measured line that survived being taken
    // away is the worst of the three: it would go on backing shares of a
    // figure the operator has withdrawn.
    let held = Declared {
        rhythm: crate::bandwidth::Rhythm::read("07:00-23:00"),
        cap: Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Pause,
        }),
        capacity: Some(Capacity {
            down: 60 * 1024 * 1024,
            up: 6 * 1024 * 1024,
            source: Source::Declared,
            taken: NOW,
            through_tunnel: false,
        }),
        ..Declared::default()
    };
    let asked = Asked {
        active: Some("none".to_owned()),
        cap: Some("off".to_owned()),
        line: Some("unset".to_owned()),
        ..Asked::default()
    };
    let cleared = revised(NOW, held, &asked).ok();
    assert!(cleared
        .as_ref()
        .is_some_and(|declared| declared.rhythm.is_none()
            && declared.cap.is_none()
            && declared.capacity.is_none()));
}

#[test]
fn a_word_this_build_does_not_know_never_falls_back_to_the_cap_s_own_answer() {
    // An operator who typed `stop` meant something by it. Keeping `continue`
    // because the new word did not read is the cap doing the opposite of what
    // they asked, at two in the morning on a stack nobody is watching — so
    // the whole request is refused rather than half of it taken.
    let held = Declared {
        cap: Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Continue,
        }),
        ..Declared::default()
    };
    let asked = Asked {
        cap: Some("1TiB".to_owned()),
        exceeded: Some("stop".to_owned()),
        ..Asked::default()
    };
    // The remedy ends where it does on purpose: the refusal for a word named
    // at a cap nobody declared adds *and only where a cap is declared* to the
    // same three words, and one was declared here — so sending the operator
    // to fix that would send them to fix something that is not wrong.
    let refused = revised(NOW, held, &asked);
    assert!(
        refused.is_err_and(|problem| {
            problem.code == UNREADABLE
                && problem.summary.contains("`stop`")
                && problem
                    .remedies
                    .first()
                    .is_some_and(|remedy| remedy.action.ends_with("pause, throttle or continue"))
        }),
        "the word they typed is quoted back, and the three it could have been"
    );
}

#[test]
fn an_override_longer_than_one_may_ask_for_is_refused_with_what_may_be() {
    let refused = revised(
        NOW,
        Declared::default(),
        &asking(|asked| asked.unrestricted_for = Some(24 * 60)),
    );
    assert!(refused.as_ref().is_err_and(|problem| {
        problem.code == UNREADABLE
            && problem
                .remedies
                .first()
                .and_then(|remedy| remedy.detail.as_deref())
                .is_some_and(|detail| detail.contains("240 minutes"))
    }));
    assert!(revised(
        NOW,
        Declared::default(),
        &asking(|asked| asked.unrestricted_for = Some(0))
    )
    .is_err());
}

#[test]
fn an_override_of_an_ordinary_evening_length_is_taken() {
    let asked = asking(|asked| asked.unrestricted_for = Some(60));
    assert_eq!(
        from_nothing(&asked)
            .ok()
            .and_then(|declared| declared.respite),
        Some(crate::bandwidth::Respite {
            until: NOW + 60 * 60
        })
    );
}

#[test]
fn what_the_line_carries_is_read_in_both_directions_at_once() {
    let asked = asking(|asked| asked.line = Some("60MiB/6MiB".to_owned()));
    let declared = from_nothing(&asked).ok().and_then(|held| held.capacity);
    assert!(declared.is_some_and(|line| line.down == 60 * 1024 * 1024
        && line.up == 6 * 1024 * 1024
        && line.taken == NOW));
}

#[test]
fn a_request_that_asks_for_nothing_changes_nothing() {
    assert_eq!(from_nothing(&Asked::default()), Ok(Declared::default()));
}

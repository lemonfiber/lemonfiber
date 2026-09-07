//! Turning what the command line accepted into what the core understands.
//!
//! A subcommand is the surface's vocabulary and a [`Command`] is the core's, and
//! this is the whole of the mapping between them. Kept apart from the dispatcher
//! so that what a request *means* can be read, and proven, without going through
//! everything that happens to it afterwards.

use lemonfiber_core::app::bundle::Wanted;
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{
    Allowance, Answer, Arranged, BandwidthAsked, Chosen, Command, Decision, Hostable, Keeping,
    QualityAction, Removing,
};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::audio::Format;
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::ports::service::{Quota, Unrated};
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::uninstall::Tier;

use crate::exit::USAGE;
use crate::say::complain;
use lemonfiber::cli::{
    Asked, ConfigAction, HostingCommand, HouseholdCommand, Kept, QualityCommand, RawAllowance,
    RawBandwidth, RawRemoval, RawRemoving, RawUnrated,
};

/// What a support bundle was asked to hold, and where it goes.
///
/// A shell has a filesystem in front of it, so a bundle asked for without a path
/// goes beside the operator rather than into a directory they would have to be
/// told about — which is the one thing this decides that a browser's request
/// cannot, and the reason the translation is not the same on both surfaces.
pub(crate) fn bundling(asked: Asked) -> Command {
    Command::Support {
        write: asked.write,
        wanted: Wanted::asked(
            asked.logs,
            asked.filenames.into(),
            asked.reveal,
            asked.confirm,
        ),
        dest: asked.out.map_or(Destination::Beside, Destination::At),
    }
}

/// Who an invitation is for, and what the account is to let them watch.
///
/// The command line spells what somebody may watch as three flags and the core carries
/// them as one choice, because they are one decision taken at one moment. Only the
/// third needs turning: libraries are named as the media server names them and an age
/// limit is the age the server already keeps, while what to do about unrated content is
/// a word here and a choice there.
///
/// **Nothing said is nothing carried.** Leaving the flag out is not choosing to let
/// unrated content through — it is saying nothing about it, which leaves the answer to
/// whatever a restriction carries by default. A `false` written for a word nobody typed
/// would be this surface deciding on the household's behalf.
pub(crate) fn invitation(name: String, allowance: RawAllowance) -> Command {
    Command::Invite {
        name,
        allowance: Allowance {
            libraries: allowance.libraries,
            age_limit: allowance.age_limit,
            unrated: allowance.unrated.map(|chosen| match chosen {
                RawUnrated::Block => Unrated::HeldBack,
                RawUnrated::Allow => Unrated::LetThrough,
            }),
        },
    }
}

/// What is being asked about the household: who is here, or what they may ask for.
///
/// One word with four things under it, because they are one subject. Naming nothing is
/// the reading; naming one of the three is a decision about what that reading shows.
///
/// **The narrowing and the decisions do not mix.** `--member` on the word itself narrows
/// the *reading* to one person, and a decision about one person carries its own — so the
/// two together are two requests in one line, and the pair is refused rather than one
/// half being dropped.
pub(crate) fn household(
    member: Option<String>,
    action: Option<HouseholdCommand>,
) -> Result<Command, u8> {
    let Some(action) = action else {
        return Ok(Command::Household { member });
    };
    if member.is_some() {
        complain!(
            "error: `--member` narrows who is listed and cannot be given to a decision \
             (name the person on the decision instead)"
        );
        return Err(USAGE);
    }
    match action {
        HouseholdCommand::Allow {
            member,
            policy,
            requests,
            days,
        } => allowing(member, policy.as_deref(), requests, days),
        HouseholdCommand::Approve { request } => Ok(Command::Deciding(Decision {
            request,
            answer: Answer::LetThrough,
        })),
        HouseholdCommand::Decline { request, reason } => Ok(Command::Deciding(Decision {
            request,
            answer: Answer::TurnedDown { reason },
        })),
        HouseholdCommand::Expiring { after, never } => {
            Ok(Command::Expiring(arranging(after, never)))
        }
    }
}

/// What was asked about what this machine keeps running.
///
/// **Naming nothing is a reading rather than an omission.** The two words underneath
/// change what this machine does at every login, and a bare word that installed
/// something would be exactly the side effect this feature refuses to be.
pub(crate) fn hosting(action: Option<HostingCommand>) -> Command {
    Command::Hosting(match action {
        None => Keeping::Read,
        Some(HostingCommand::Install { what, forms }) => Keeping::Install {
            what: kept(what),
            forms,
        },
        Some(HostingCommand::Remove { what }) => Keeping::Remove { what: kept(what) },
    })
}

/// The command line's word for one of them, as the core names it.
const fn kept(what: Kept) -> Hostable {
    match what {
        Kept::Watch => Hostable::Watch,
        Kept::Expiring => Hostable::Expiring,
    }
}

/// What is being arranged about the requests nobody rules on.
///
/// **Naming nothing is a request in its own right rather than an omission**, and it is the
/// one that runs: an operator who has already said how long is too long is not saying it
/// again to act on it. That is why there is no shape here for a default — a period this
/// invented would close somebody's request on lemonfiber's authority, and the run that
/// names none is asking to act on the household's own.
const fn arranging(after: Option<u32>, never: bool) -> Arranged {
    match (after, never) {
        (Some(days), _) => Arranged::After(days),
        (None, true) => Arranged::Never,
        (None, false) => Arranged::AsAgreed,
    }
}

/// What the household is to be allowed to ask for, from the words it was chosen in.
///
/// The policy is a word here and a value there; the limit is two numbers that only mean
/// something together, which is why the command line refuses either without the other
/// before this is reached. A word this build does not know is refused by name rather
/// than falling to whichever policy is safer — somebody who wrote a word and meant it
/// must not be given a different arrangement because of a spelling.
fn allowing(
    member: Option<String>,
    policy: Option<&str>,
    requests: Option<u32>,
    days: Option<u32>,
) -> Result<Command, u8> {
    let mut chosen = None;
    if let Some(written) = policy {
        let Some(named) = Policy::from_label(written) else {
            complain!(
                "error: no policy named `{written}` (try {})",
                Policy::labels()
            );
            return Err(USAGE);
        };
        chosen = Some(named);
    }
    Ok(Command::Allowing(Chosen {
        member,
        policy: chosen,
        quota: requests
            .zip(days)
            .map(|(requests, days)| Quota { requests, days }),
    }))
}

/// Which setting the operator is reading or changing.
pub(crate) fn configuration(action: ConfigAction) -> Command {
    match action {
        ConfigAction::Get { key } => Command::ConfigGet { key },
        ConfigAction::Set { key, value } => Command::ConfigSet { key, value },
        ConfigAction::Show => Command::ConfigShow,
    }
}

pub(crate) fn quality(action: QualityCommand) -> Result<Command, u8> {
    let action = match action {
        QualityCommand::Show => QualityAction::Show,
        QualityCommand::Set {
            preset,
            media_type,
            confirm,
        } => {
            // Music has no resolution: `--for music` chooses an audio format instead of a
            // resolution preset, and reaches the service rather than only recording, so it
            // routes to its own command.
            if media_type.as_deref() == Some("music") {
                let Some(format) = Format::from_label(&preset) else {
                    complain!(
                        "error: no music format named `{preset}` \
                         (try compact, lossless, or hi-res)"
                    );
                    return Err(USAGE);
                };
                return Ok(Command::QualityMusic { format });
            }
            let Some(preset) = Preset::from_label(&preset) else {
                complain!(
                    "error: no quality preset named `{preset}` \
                     (try space-saving, balanced, high-quality, or maximum)"
                );
                return Err(USAGE);
            };
            if let Some(media_type) = &media_type {
                if !Kind::ALL.iter().any(|kind| kind.media_type() == media_type) {
                    complain!(
                        "error: no media type named `{media_type}` (try tv, movies, or music)"
                    );
                    return Err(USAGE);
                }
            }
            QualityAction::Set {
                preset,
                media_type,
                confirm,
            }
        }
        QualityCommand::Reapply => QualityAction::Reapply,
        // Upgrade is its own command, not a quality action, since it reaches the
        // services rather than only reading or writing the recorded choice.
        QualityCommand::Upgrade { confirm } => return Ok(Command::QualityUpgrade { confirm }),
    };
    Ok(Command::Quality(action))
}

/// What a trace is asked about, from the words it was typed as.
///
/// The term is taken as words so it can be typed unquoted; joined back into the title
/// as said. The searching form is the one that reaches past this machine, spending a
/// real search against the indexers' daily allowance, so it happens only where the
/// flag asked for it.
pub(crate) fn traced(term: &[String], season: Option<u32>, search: bool) -> Command {
    Command::Trace {
        term: term.join(" "),
        season,
        searching: search,
    }
}

/// What was asked about the line, carried as it was written.
///
/// Not one value is interpreted here. What `50%` means, what may be lifted for how
/// long, and what a cap needs beside it are all decisions the core makes for every
/// surface at once — a shell that read them would be a second answer to the same
/// question, and the two would part company on the first change to either.
pub(crate) fn sharing(asked: RawBandwidth) -> Command {
    Command::Bandwidth(BandwidthAsked {
        down: asked.down,
        up: asked.up,
        active: asked.active,
        line: asked.line,
        cap: asked.cap,
        exceeded: asked.when_exceeded,
        unrestricted_for: asked.unrestricted_for,
    })
}

/// A restart of named services, or of everything the form holds where none are named.
pub(crate) fn restarting(form: String, services: Vec<String>) -> Command {
    Command::Restart {
        forms: vec![form],
        services,
    }
}

/// Which completed download to stop seeding, and the offer being answered.
///
/// A rename and nothing else, which is what makes it worth writing down: the command
/// line calls the answer `--offer`, because what an operator types is the name the run
/// before it printed, and the core calls it the agreement, because what it does with
/// it is compare it against the offer standing now. One word for one thing on each
/// side, and this is the whole of the join.
///
/// Nothing typed is nothing agreed to. A flag given empty is an answer to no offer,
/// and carrying it would be a name the core goes and fails to match, so it is dropped
/// here, where the emptiness is visible, rather than travelling as one.
pub(crate) fn letting(download: String, offer: Option<String>) -> Command {
    Command::StopSeeding {
        download,
        agreement: offer.filter(|named| !named.trim().is_empty()),
    }
}

/// Which removal was asked for, and what was answered about it.
///
/// The four words are a `ValueEnum` on this surface, so a fifth word is refused by
/// the parser before anything here runs — which is why this translation cannot fail
/// and the core's own refusal for an unrecognised removal belongs to the surface that
/// takes one as free text.
///
/// Nothing typed is nothing agreed to, for the reason [`letting`] drops an empty
/// offer: a flag given empty is an answer to no listing, and carrying it would be a
/// name the core goes and fails to match.
pub(crate) fn removing(asked: RawRemoving) -> Command {
    let tier = match asked.tier {
        RawRemoval::Stop => Tier::Stop,
        RawRemoval::Services => Tier::Services,
        RawRemoval::Configuration => Tier::Configuration,
        RawRemoval::Media => Tier::Media,
    };
    Command::Uninstall(
        Removing::surveying(tier)
            .confirmed(asked.confirm)
            .agreeing(asked.agreed.filter(|named| !named.trim().is_empty()))
            .waiting(asked.wait.into()),
    )
}

/// The diagnosis a plain run asks for, narrowed as it was asked to be.
///
/// Named apart because the arm it came from carries a fork of its own — a run that
/// mends returns before this is reached — and the two together are longer than the
/// table has room for.
pub(crate) fn diagnosing(
    only: Option<&str>,
    disruptive: bool,
    accept: Option<String>,
) -> Result<Command, u8> {
    narrowed(only).map(|narrowing| Command::Doctor {
        narrowing,
        disruptive,
        accept,
    })
}

/// What a diagnosis was narrowed to, or the code to exit with for a name that is
/// neither a category nor a check inside one.
///
/// A name lemonfiber does not know is a mistake to correct rather than a request to
/// run everything — refused here, before the core is reached. Whether a stack reports
/// the check named is a question only the run can answer, and it answers it.
fn narrowed(only: Option<&str>) -> Result<Narrowing, u8> {
    match only.map(Narrowing::parse) {
        Some(None) => {
            let named = only.unwrap_or_default();
            complain!("error: no diagnostic category or check named `{named}`");
            Err(USAGE)
        }
        Some(Some(narrowing)) => Ok(narrowing),
        None => Ok(Narrowing::Suite),
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::app::{Allowance, Command, QualityAction};
    use lemonfiber_core::audio::Format;
    use lemonfiber_core::quality::Preset;

    use super::{
        bundling, configuration, hosting, household, invitation, letting, quality, restarting,
        sharing, traced, Answer, Arranged, Chosen, Decision, Destination, Hostable, Keeping,
        Policy, Quota, Wanted,
    };
    use super::{diagnosing, narrowed};
    use crate::exit::USAGE;
    use lemonfiber::cli::{
        Asked, ConfigAction, HostingCommand, HouseholdCommand, Kept, QualityCommand, RawAllowance,
        RawBandwidth, RawUnrated,
    };
    use lemonfiber_core::app::BandwidthAsked;
    use lemonfiber_core::bundle::Filenames;
    use lemonfiber_core::doctor::Narrowing;
    use lemonfiber_core::ports::service::Unrated;

    /// One choice about what the household may ask for, as the command line took it.
    fn allowing(
        member: Option<&str>,
        policy: Option<&str>,
        requests: Option<u32>,
        days: Option<u32>,
    ) -> Result<Command, u8> {
        household(
            None,
            Some(HouseholdCommand::Allow {
                member: member.map(str::to_owned),
                policy: policy.map(str::to_owned),
                requests,
                days,
            }),
        )
    }

    /// Naming nothing under the word is the reading, narrowed or whole.
    #[test]
    fn naming_nothing_under_the_word_is_the_reading() {
        assert_eq!(
            household(None, None),
            Ok(Command::Household { member: None })
        );
        assert_eq!(
            household(Some("ana".to_owned()), None),
            Ok(Command::Household {
                member: Some("ana".to_owned())
            })
        );
    }

    /// The narrowing and a decision are two requests in one line, so the pair is
    /// refused rather than one half being dropped.
    #[test]
    fn the_narrowing_and_a_decision_are_refused_together() {
        assert_eq!(
            household(
                Some("ana".to_owned()),
                Some(HouseholdCommand::Approve { request: 7 })
            ),
            Err(USAGE)
        );
    }

    /// A policy and a limit reach the choice as one value, for the house or one person.
    #[test]
    fn a_policy_and_a_limit_reach_the_choice_as_one_value() {
        assert_eq!(
            allowing(Some("ana"), Some("within-a-limit"), Some(5), Some(7)),
            Ok(Command::Allowing(Chosen {
                member: Some("ana".to_owned()),
                policy: Some(Policy::WithinALimit),
                quota: Some(Quota {
                    requests: 5,
                    days: 7
                }),
            }))
        );
    }

    /// Saying nothing about something carries nothing.
    ///
    /// A run that named only a limit is not a run that chose to trust everybody, and a
    /// value written here for a word nobody typed would be this surface deciding on the
    /// household's behalf.
    #[test]
    fn saying_nothing_about_something_carries_nothing() {
        assert_eq!(
            allowing(None, None, Some(3), Some(30)),
            Ok(Command::Allowing(Chosen {
                member: None,
                policy: None,
                quota: Some(Quota {
                    requests: 3,
                    days: 30
                }),
            }))
        );
        assert_eq!(
            allowing(None, Some("trusted"), None, None),
            Ok(Command::Allowing(Chosen {
                member: None,
                policy: Some(Policy::Trusted),
                quota: None,
            }))
        );
    }

    /// A policy this build does not know is a usage error naming the ones there are.
    #[test]
    fn a_policy_this_build_does_not_know_is_a_usage_error() {
        assert_eq!(allowing(None, Some("generous"), None, None), Err(USAGE));
    }

    /// Approving and declining are the same command with different answers, and only
    /// one of them carries a reason.
    #[test]
    fn approving_and_declining_are_one_command_with_two_answers() {
        assert_eq!(
            household(None, Some(HouseholdCommand::Approve { request: 7 })),
            Ok(Command::Deciding(Decision {
                request: 7,
                answer: Answer::LetThrough,
            }))
        );
        assert_eq!(
            household(
                None,
                Some(HouseholdCommand::Decline {
                    request: 8,
                    reason: "no room".to_owned(),
                })
            ),
            Ok(Command::Deciding(Decision {
                request: 8,
                answer: Answer::TurnedDown {
                    reason: "no room".to_owned()
                },
            }))
        );
    }

    /// Arranging an expiry, withdrawing it, and running on it are three requests.
    ///
    /// **Naming nothing is the one that acts**, and it is a request rather than an
    /// omission: an operator who has already said how long is too long is not saying it
    /// again in order to act on it, and there is no period this surface would supply for
    /// a run that named none.
    #[test]
    fn arranging_an_expiry_and_running_on_it_are_different_requests() {
        assert_eq!(
            household(
                None,
                Some(HouseholdCommand::Expiring {
                    after: Some(30),
                    never: false,
                })
            ),
            Ok(Command::Expiring(Arranged::After(30)))
        );
        assert_eq!(
            household(
                None,
                Some(HouseholdCommand::Expiring {
                    after: None,
                    never: true,
                })
            ),
            Ok(Command::Expiring(Arranged::Never))
        );
        assert_eq!(
            household(
                None,
                Some(HouseholdCommand::Expiring {
                    after: None,
                    never: false,
                })
            ),
            Ok(Command::Expiring(Arranged::AsAgreed))
        );
    }

    /// Three flags at the command line are one choice in the core.
    #[test]
    fn an_invitation_carries_what_was_chosen_as_one_allowance() {
        assert_eq!(
            invitation(
                "ada".to_owned(),
                RawAllowance {
                    libraries: vec!["Films".to_owned()],
                    age_limit: Some(12),
                    unrated: Some(RawUnrated::Block),
                }
            ),
            Command::Invite {
                name: "ada".to_owned(),
                allowance: Allowance {
                    libraries: vec!["Films".to_owned()],
                    age_limit: Some(12),
                    unrated: Some(Unrated::HeldBack),
                },
            }
        );
    }

    /// Every flag reaches the core as it was written, and none of it is read here.
    ///
    /// A shell that read `50%` would be a second answer to what a share means, and
    /// the two would part company on the first change to either — which a
    /// household would meet as an evening that went wrong on one surface and not
    /// on another.
    #[test]
    fn what_was_asked_about_the_line_reaches_the_core_unread() {
        assert_eq!(
            sharing(RawBandwidth {
                down: Some("50%".to_owned()),
                up: Some("2MiB".to_owned()),
                active: Some("07:00-23:00".to_owned()),
                line: Some("60MiB/6MiB".to_owned()),
                cap: Some("1TiB".to_owned()),
                when_exceeded: Some("pause".to_owned()),
                unrestricted_for: Some(60),
            }),
            Command::Bandwidth(BandwidthAsked {
                down: Some("50%".to_owned()),
                up: Some("2MiB".to_owned()),
                active: Some("07:00-23:00".to_owned()),
                line: Some("60MiB/6MiB".to_owned()),
                cap: Some("1TiB".to_owned()),
                exceeded: Some("pause".to_owned()),
                unrestricted_for: Some(60),
            })
        );
    }

    /// Asked nothing, it asks the core for nothing — which is the reading.
    #[test]
    fn a_line_asked_about_and_not_changed_carries_no_answers() {
        assert_eq!(
            sharing(RawBandwidth {
                down: None,
                up: None,
                active: None,
                line: None,
                cap: None,
                when_exceeded: None,
                unrestricted_for: None,
            }),
            Command::Bandwidth(BandwidthAsked::default())
        );
    }

    /// Naming no service restarts whatever the form holds, which is the form alone.
    #[test]
    fn a_restart_carries_the_one_form_and_only_the_services_named() {
        assert_eq!(
            restarting("media".to_owned(), vec!["sonarr".to_owned()]),
            Command::Restart {
                forms: vec!["media".to_owned()],
                services: vec!["sonarr".to_owned()],
            }
        );
        assert_eq!(
            restarting("media".to_owned(), Vec::new()),
            Command::Restart {
                forms: vec!["media".to_owned()],
                services: Vec::new(),
            }
        );
    }

    /// The words are the title, and the search happens only where it was asked for.
    ///
    /// A term typed unquoted arrives as words and is one title again; the flag is what
    /// separates a trace that reads from one that spends a real search.
    #[test]
    fn a_trace_joins_its_words_and_searches_only_when_asked() {
        assert_eq!(
            traced(&["the".to_owned(), "wire".to_owned()], Some(2), true),
            Command::Trace {
                term: "the wire".to_owned(),
                season: Some(2),
                searching: true,
            }
        );
        assert_eq!(
            traced(&["dune".to_owned()], None, false),
            Command::Trace {
                term: "dune".to_owned(),
                season: None,
                searching: false,
            }
        );
    }

    #[test]
    fn each_configuration_action_becomes_its_own_command() {
        assert_eq!(
            configuration(ConfigAction::Get {
                key: "DATA_ROOT".to_owned()
            }),
            Command::ConfigGet {
                key: "DATA_ROOT".to_owned()
            }
        );
        assert_eq!(
            configuration(ConfigAction::Set {
                key: "DATA_ROOT".to_owned(),
                value: "/srv".to_owned()
            }),
            Command::ConfigSet {
                key: "DATA_ROOT".to_owned(),
                value: "/srv".to_owned()
            }
        );
        assert_eq!(configuration(ConfigAction::Show), Command::ConfigShow);
    }

    #[test]
    fn showing_and_reapplying_the_quality_choice_need_no_argument() {
        assert_eq!(
            quality(QualityCommand::Show),
            Ok(Command::Quality(QualityAction::Show))
        );
        assert_eq!(
            quality(QualityCommand::Reapply),
            Ok(Command::Quality(QualityAction::Reapply))
        );
    }

    #[test]
    fn a_preset_can_be_set_for_everything_or_for_one_media_type() {
        let everything = quality(QualityCommand::Set {
            preset: "balanced".to_owned(),
            media_type: None,
            confirm: false,
        });
        assert!(matches!(
            everything,
            Ok(Command::Quality(QualityAction::Set {
                preset: Preset::Balanced,
                media_type: None,
                confirm: false
            }))
        ));
        let television = quality(QualityCommand::Set {
            preset: "maximum".to_owned(),
            media_type: Some("tv".to_owned()),
            confirm: true,
        });
        assert!(matches!(
            television,
            Ok(Command::Quality(QualityAction::Set {
                preset: Preset::Maximum,
                confirm: true,
                ..
            }))
        ));
    }

    #[test]
    fn a_music_format_is_its_own_command_rather_than_a_preset() {
        // Music has no resolution, so it is set by format and reaches the service
        // asynchronously — a different command, not a variant of the same one.
        assert!(matches!(
            quality(QualityCommand::Set {
                preset: "lossless".to_owned(),
                media_type: Some("music".to_owned()),
                confirm: false,
            }),
            Ok(Command::QualityMusic {
                format: Format::Lossless
            })
        ));
    }

    #[test]
    fn upgrading_existing_content_is_its_own_command() {
        // It reaches the services asynchronously rather than recording a choice, so
        // it is not a quality action but a command of its own.
        assert_eq!(
            quality(QualityCommand::Upgrade { confirm: true }),
            Ok(Command::QualityUpgrade { confirm: true })
        );
    }

    #[test]
    fn a_preset_or_media_type_this_build_does_not_know_is_a_usage_error() {
        // Named rather than guessed at: the operator gets to see what was expected.
        for (preset, media_type) in [
            ("gorgeous", None),
            ("balanced", Some("audiobooks".to_owned())),
            ("mp3", Some("music".to_owned())),
        ] {
            assert_eq!(
                quality(QualityCommand::Set {
                    preset: preset.to_owned(),
                    media_type,
                    confirm: false,
                }),
                Err(USAGE)
            );
        }
    }

    #[test]
    fn a_bundle_asked_for_at_a_shell_goes_where_the_shell_is() {
        // The one thing this decides that a browser's request cannot: a shell has a
        // filesystem in front of it, so a bundle with no path named goes beside the
        // operator rather than into a directory they would have to be told about.
        let asked = Asked {
            write: true,
            logs: 12,
            filenames: true,
            reveal: vec!["INDEXER_KEY".to_owned()],
            confirm: true,
            out: None,
        };
        assert_eq!(
            bundling(asked),
            Command::Support {
                write: true,
                wanted: Wanted::asked(12, Filenames::Shown, vec!["INDEXER_KEY".to_owned()], true),
                dest: Destination::Beside,
            }
        );
    }

    #[test]
    fn a_bundle_told_where_to_go_goes_there() {
        let asked = Asked {
            write: true,
            logs: 50,
            filenames: false,
            reveal: Vec::new(),
            confirm: false,
            out: Some(std::path::PathBuf::from("/tmp/bundle.tar.gz")),
        };
        assert!(matches!(
            bundling(asked),
            Command::Support {
                dest: Destination::At(path),
                ..
            } if path == std::path::Path::new("/tmp/bundle.tar.gz")
        ));
    }

    /// One invitation, told what to do about unrated content or told nothing.
    fn offering(unrated: Option<RawUnrated>) -> Command {
        invitation(
            "ana".to_owned(),
            RawAllowance {
                libraries: vec!["Films".to_owned()],
                age_limit: Some(12),
                unrated,
            },
        )
    }

    /// The same invitation as it reaches the core, told what to do or told nothing.
    fn reaching(unrated: Option<Unrated>) -> Command {
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance {
                libraries: vec!["Films".to_owned()],
                age_limit: Some(12),
                unrated,
            },
        }
    }

    /// The other word reaches the other answer, so the two cannot be one flag read
    /// twice — the case above carries the first, and each has to reach its own.
    #[test]
    fn the_other_word_reaches_the_other_answer() {
        assert_eq!(
            offering(Some(RawUnrated::Allow)),
            reaching(Some(Unrated::LetThrough))
        );
    }

    /// Saying nothing about unrated content carries nothing.
    ///
    /// Leaving the flag out is not choosing to let it through; it is saying nothing,
    /// which leaves the answer to whatever a restriction carries by default. A value
    /// written here for a word nobody typed would be this surface deciding on the
    /// household's behalf.
    #[test]
    fn saying_nothing_about_unrated_content_carries_nothing() {
        assert_eq!(offering(None), reaching(None));
    }

    /// The download every stopping case here names.
    const HELD: &str = "A.Show.S01E01";

    /// Named alone, it asks what stopping it would cost and agrees to nothing.
    #[test]
    fn a_download_named_alone_asks_what_stopping_it_would_cost() {
        assert_eq!(
            letting(HELD.to_owned(), None),
            Command::StopSeeding {
                download: HELD.to_owned(),
                agreement: None
            }
        );
    }

    /// The offer typed back is the agreement the core compares against what stands.
    #[test]
    fn the_offer_typed_back_is_the_agreement_the_core_compares() {
        assert_eq!(
            letting(HELD.to_owned(), Some("3f2a1b9c".to_owned())),
            Command::StopSeeding {
                download: HELD.to_owned(),
                agreement: Some("3f2a1b9c".to_owned())
            }
        );
    }

    /// An empty answer is no answer, rather than one the core goes and fails to match.
    #[test]
    fn an_empty_answer_is_no_answer_at_all() {
        assert_eq!(
            letting(HELD.to_owned(), Some("   ".to_owned())),
            Command::StopSeeding {
                download: HELD.to_owned(),
                agreement: None
            }
        );
    }

    /// Asking about what is hosted is a reading, and only the words underneath change it.
    #[test]
    fn a_bare_word_reads_and_does_not_install_anything() {
        assert_eq!(hosting(None), Command::Hosting(Keeping::Read));
    }

    /// Both of the two long commands are reachable through one word, and the guard
    /// carries the forms it was named against.
    #[test]
    fn either_of_them_can_be_installed_and_taken_back_again() {
        assert_eq!(
            hosting(Some(HostingCommand::Install {
                what: Kept::Watch,
                forms: vec!["tv".to_owned()],
            })),
            Command::Hosting(Keeping::Install {
                what: Hostable::Watch,
                forms: vec!["tv".to_owned()],
            })
        );
        assert_eq!(
            hosting(Some(HostingCommand::Install {
                what: Kept::Expiring,
                forms: Vec::new(),
            })),
            Command::Hosting(Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new(),
            })
        );
        assert_eq!(
            hosting(Some(HostingCommand::Remove { what: Kept::Watch })),
            Command::Hosting(Keeping::Remove {
                what: Hostable::Watch
            })
        );
        assert_eq!(
            hosting(Some(HostingCommand::Remove {
                what: Kept::Expiring
            })),
            Command::Hosting(Keeping::Remove {
                what: Hostable::Expiring
            })
        );
    }

    /// Naming no category at all asks for the whole suite rather than for nothing.
    #[test]
    fn naming_no_category_asks_for_the_whole_suite() {
        assert_eq!(narrowed(None), Ok(Narrowing::Suite));
    }

    /// A check inside a category is narrowed to that check, under the name given.
    #[test]
    fn a_check_inside_a_category_is_narrowed_to_that_check() {
        assert_eq!(
            narrowed(Some("storage.space")),
            Ok(Narrowing::Check("storage.space".to_owned()))
        );
    }

    /// A name that is neither a category nor a check inside one is a usage error, so
    /// that a mistyped narrowing is corrected rather than quietly running everything.
    #[test]
    fn a_name_that_is_neither_a_category_nor_a_check_is_a_usage_error() {
        assert_eq!(narrowed(Some("nonsense")), Err(USAGE));
    }

    /// What a diagnosis was narrowed to reaches the command alongside what it was
    /// asked to accept and whether it may disrupt.
    #[test]
    fn a_diagnosis_carries_what_it_was_narrowed_to() {
        assert_eq!(
            diagnosing(Some("storage.space"), true, Some("STORAGE-1".to_owned())),
            Ok(Command::Doctor {
                narrowing: Narrowing::Check("storage.space".to_owned()),
                disruptive: true,
                accept: Some("STORAGE-1".to_owned()),
            })
        );
    }

    /// A removal as the command line accepted it.
    fn removing_asked(
        tier: lemonfiber::cli::RawRemoval,
        agreed: Option<&str>,
        wait: bool,
    ) -> lemonfiber::cli::RawRemoving {
        lemonfiber::cli::RawRemoving {
            tier,
            confirm: true,
            agreed: agreed.map(str::to_owned),
            wait,
        }
    }

    /// Each of the four words reaches the removal it names, and nothing else.
    ///
    /// Compared whole rather than field by field: what this is about is that one
    /// word becomes one command, and a comparison that read only the removal out of
    /// it would pass on a translation that had dropped everything else.
    #[test]
    fn each_word_reaches_the_removal_it_names() {
        use lemonfiber::cli::RawRemoval;
        use lemonfiber_core::app::Removing;
        use lemonfiber_core::uninstall::Tier;

        let pairs = [
            (RawRemoval::Stop, Tier::Stop),
            (RawRemoval::Services, Tier::Services),
            (RawRemoval::Configuration, Tier::Configuration),
            (RawRemoval::Media, Tier::Media),
        ];
        for (written, meant) in pairs {
            assert_eq!(
                super::removing(removing_asked(written, None, false)),
                Command::Uninstall(Removing::surveying(meant).confirmed(true)),
                "{written:?}"
            );
        }
    }

    /// Nothing typed is nothing agreed to: a flag given empty is an answer to no
    /// listing, and carrying it would be a name the core goes and fails to match.
    #[test]
    fn an_empty_answer_to_a_listing_is_dropped_rather_than_carried() {
        use lemonfiber::cli::RawRemoval;
        use lemonfiber_core::app::Removing;
        use lemonfiber_core::uninstall::Tier;

        let media = || Removing::surveying(Tier::Media).confirmed(true);

        assert_eq!(
            super::removing(removing_asked(RawRemoval::Media, Some("  "), false)),
            Command::Uninstall(media())
        );
        assert_eq!(
            super::removing(removing_asked(RawRemoval::Media, Some("deadbeef"), false)),
            Command::Uninstall(media().agreeing(Some("deadbeef".to_owned())))
        );
    }

    /// The wait is carried as the word the core knows it by rather than as a flag.
    #[test]
    fn asking_a_removal_to_wait_reaches_it_as_a_wait() {
        use lemonfiber::cli::RawRemoval;
        use lemonfiber_core::app::{Removing, Waiting};
        use lemonfiber_core::uninstall::Tier;

        let stopping = || Removing::surveying(Tier::Stop).confirmed(true);

        assert_eq!(
            super::removing(removing_asked(RawRemoval::Stop, None, true)),
            Command::Uninstall(stopping().waiting(Waiting::ForTheDownloads))
        );
        assert_eq!(
            super::removing(removing_asked(RawRemoval::Stop, None, false)),
            Command::Uninstall(stopping())
        );
    }
}

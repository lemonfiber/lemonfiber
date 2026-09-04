//! Turning what the command line accepted into what the core understands.
//!
//! A subcommand is the surface's vocabulary and a [`Command`] is the core's, and
//! this is the whole of the mapping between them. Kept apart from the dispatcher
//! so that what a request *means* can be read, and proven, without going through
//! everything that happens to it afterwards.

use lemonfiber::cli::RawAllowance;
use lemonfiber_core::app::bundle::Wanted;
use lemonfiber_core::app::support::Destination;
use lemonfiber_core::app::{Allowance, Command, QualityAction};
use lemonfiber_core::audio::Format;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;

use crate::exit::USAGE;
use crate::say::complain;
use lemonfiber::cli::{Asked, ConfigAction, QualityCommand};

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

/// Who an invitation is for, and what the account is to let them watch.
///
/// The command line spells what somebody may
/// watch as two flags and the core carries them as one choice,, so the two are put together here.
pub(crate) fn invitation(name: String, allowance: RawAllowance) -> Command {
    Command::Invite {
        name,
        allowance: Allowance {
            libraries: allowance.libraries,
            age_limit: allowance.age_limit,
        },
    }
}

/// A restart of named services, or of everything the form holds where none are named.
pub(crate) fn restarting(form: String, services: Vec<String>) -> Command {
    Command::Restart {
        forms: vec![form],
        services,
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber::cli::RawAllowance;
    use lemonfiber_core::app::{Allowance, Command, QualityAction};
    use lemonfiber_core::audio::Format;
    use lemonfiber_core::quality::Preset;

    use super::{
        bundling, configuration, invitation, quality, restarting, traced, Allowance, Destination,
        RawAllowance, Wanted,
    };
    use crate::exit::USAGE;
    use lemonfiber::cli::{Asked, ConfigAction, QualityCommand};
    use lemonfiber_core::bundle::Filenames;

    /// Two flags at the command line are one choice in the core.
    #[test]
    fn an_invitation_carries_what_was_chosen_as_one_allowance() {
        assert_eq!(
            invitation(
                "ada".to_owned(),
                RawAllowance {
                    libraries: vec!["Films".to_owned()],
                    age_limit: Some(12),
                }
            ),
            Command::Invite {
                name: "ada".to_owned(),
                allowance: Allowance {
                    libraries: vec!["Films".to_owned()],
                    age_limit: Some(12),
                },
            }
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
}

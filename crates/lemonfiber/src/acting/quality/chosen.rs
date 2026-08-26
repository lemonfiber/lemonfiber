//! What a quality choice is made of: the media it is about, and the bar it aims for.
//!
//! The vocabulary of one choice, apart from the flow that makes it. [`super`] decides
//! which change is being made, what is put in front of each question and what the yes
//! sends; this is what the two lists in the middle of that are made of, and what the
//! pair of answers is called once both have been taken.
//!
//! **Two lists rather than one, because the second depends on the first.** Music has
//! no resolution: choosing for it picks an audio format and reaches a different
//! command, which is the fork `--for music` takes on a command line and the fork
//! [`lemonfiber_api::actions::named`] takes on a request body. So what a bar even *is*
//! is decided by the media, and a single list of every pairing would be fifteen rows
//! for a question that is two.
//!
//! **And the fork is read rather than written down.** Every bar of both kinds is put
//! to the translation for each media in turn, and what comes back as a command is what
//! is offered. That is the whole of why music shows three audio formats where the rest
//! show four resolution presets: the table refuses a preset named for music and an
//! audio format named for anything else, so this screen never has to know which is
//! which — and a media type the table stopped accepting would stop being offered here
//! without anybody editing a list.

use lemonfiber_api::actions::Arguments;
use lemonfiber_core::audio::Format;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::recyclarr::Kind;

use super::super::chooser::Listed;

/// What a choice is about: the whole library, or one kind of media in it.
///
/// The whole library first, because it is what the choice has always meant here and
/// what most operators want; the kinds after it, in the order the quality model lists
/// them, with music last because it is the one that is not a resolution at all.
pub(crate) struct Scope {
    /// What it is called on the list.
    pub(crate) name: &'static str,
    /// What choosing it comes to, in the line beside the name.
    about: String,
    /// The media type it names, or nothing for the whole library.
    pub(super) media_type: Option<&'static str>,
}

impl Listed for Scope {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        &self.about
    }
}

/// What music is called wherever a media type is named.
///
/// Music is not one of the two services the resolution presets configure, so no
/// [`Kind`] names it — and it is exactly the media whose quality is not a resolution,
/// which is why naming it forks the action inside the translation. The word is the
/// one that translation and the command line both take.
const MUSIC: &str = "music";

/// What the whole library is called where it is one of the choices.
const WHOLE_LIBRARY: &str = "everything";

impl Scope {
    /// The whole library, which is what naming no media type at all means.
    pub(super) fn everything() -> Self {
        Self {
            name: WHOLE_LIBRARY,
            about: "every kind of media this stack fetches, rather than one of them".to_owned(),
            media_type: None,
        }
    }

    /// One of the kinds whose quality is a resolution, in the core's own word for it.
    pub(super) fn kind(kind: Kind) -> Self {
        Self {
            name: kind.noun(),
            about: format!("{} only, leaving every other kind as it is", kind.noun()),
            media_type: Some(kind.media_type()),
        }
    }

    /// Music, whose quality is an audio format rather than a resolution.
    pub(super) fn music() -> Self {
        Self {
            name: MUSIC,
            about: "music only, which is chosen as an audio format because music has no \
                    resolution"
                .to_owned(),
            media_type: Some(MUSIC),
        }
    }
}

/// One preset, and what choosing it comes to.
pub(crate) struct Grade {
    /// The plain-language name the preset is chosen and stored by.
    pub(crate) name: &'static str,
    /// What it means, and roughly what an hour of it costs.
    pub(crate) about: String,
}

impl Listed for Grade {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        &self.about
    }
}

impl Grade {
    /// One bar as a row: its own name, and what it means beside what it costs.
    ///
    /// The two halves are joined as one line rather than left as a sentence followed
    /// by a fragment, since a row has one line to say both in.
    ///
    /// Three strings rather than a preset, because the two things this list is ever
    /// made of answer the same three questions and are otherwise unalike: a
    /// resolution preset and an audio format have no type in common and no reason to
    /// grow one.
    fn of(name: &'static str, means: &'static str, costs: &'static str) -> Self {
        Self {
            name,
            about: format!("{} — {costs}.", means.trim_end_matches('.')),
        }
    }

    /// Every bar this screen could offer, whichever kind of media a scope names.
    ///
    /// Both kinds every time, and the translation drops the ones that do not belong.
    /// A resolution preset named for music is refused there and an audio format named
    /// for anything else is refused there too, so the fork the action takes is read
    /// off the table that takes it rather than written down a second time here.
    pub(super) fn every() -> impl Iterator<Item = Self> {
        Preset::ALL
            .into_iter()
            .map(|preset| {
                Self::of(
                    preset.label(),
                    preset.means(),
                    preset.consequence().size_per_hour,
                )
            })
            .chain(Format::ALL.into_iter().map(|format| {
                Self::of(
                    format.label(),
                    format.means(),
                    format.consequence().size_per_hour,
                )
            }))
    }
}

/// What a change was chosen, and what the question calls it.
///
/// The two halves of a choice held together — the media it is about and the bar it
/// aims for — because they are chosen one after the other and every step after the
/// second carries both. Two fields travelling separately through three stages are two
/// fields that eventually arrive apart, which on this action would mean agreeing to a
/// bar for one kind of media and recording it for another.
pub(crate) struct Chosen {
    /// The media type it applies to, or nothing for the whole library.
    about: Option<&'static str>,
    /// The bar it aims for, or nothing where the change takes none.
    grade: Option<&'static str>,
    /// What the question calls the pair, empty where the change was chosen nothing.
    said: String,
}

impl Chosen {
    /// The arguments this choice fills, before the agreement goes on.
    ///
    /// Handed over as the action's own arguments rather than as two fields to be read
    /// off, so what a choice *is* stays here and what is done with it stays next door.
    pub(super) fn asked(&self) -> Arguments {
        Arguments {
            preset: self.grade.map(str::to_owned),
            media_type: self.about.map(str::to_owned),
            ..Arguments::default()
        }
    }
}

impl Chosen {
    /// A change that takes neither: re-asserting what is recorded, and fetching the
    /// library again at it.
    pub(crate) fn nothing() -> Self {
        Self {
            about: None,
            grade: None,
            said: String::new(),
        }
    }

    /// A choice about the whole library, before a bar has been taken for it.
    #[cfg(test)]
    pub(super) fn everywhere() -> Self {
        Self::media(&Scope::everything())
    }

    /// The media a choice is about, before a bar has been taken for it.
    pub(super) fn media(scope: &Scope) -> Self {
        Self {
            about: scope.media_type,
            grade: None,
            said: scope.name.to_owned(),
        }
    }

    /// The same, with the bar that was taken off the list for it.
    ///
    /// The media is said beside the bar rather than left behind, because the question
    /// is the last place either is stated and "aim for balanced" is a different
    /// request from "aim for balanced, for series".
    pub(super) fn graded(&self, grade: &'static str) -> Self {
        let said = match self.about {
            None => format!("{grade}, everywhere"),
            Some(_) => format!("{grade}, for {}", self.said),
        };
        Self {
            about: self.about,
            grade: Some(grade),
            said,
        }
    }

    /// What the question calls it.
    pub(crate) fn said(&self) -> &str {
        &self.said
    }
}

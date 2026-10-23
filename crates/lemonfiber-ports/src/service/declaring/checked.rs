//! Whether a declaration is one a guard can enforce: nothing in it sends a request, or a
//! member's token, anywhere but the upstream or the adapter, every location it gives a
//! member is on the household address, and every pattern in it is one a linear-time
//! engine compiles within a bound and that is anchored where the guard relies on it.
//!
//! Every check here is an allowlist, and every path and template is read by
//! [`joined`], the one reading the guard's question and the core's locations are built
//! with.

use regex_syntax::hir::Look;

use super::joined::joined;
use super::{
    Answers, BytePath, Declared, Held, LivePath, Locations, Presentation, Question, Statuses,
    Token, BASE, ITEM, TOKEN,
};

/// The longest pattern a declaration may hold, in bytes.
const PATTERN_MOST: usize = 1024;

/// The most a compiled pattern may take, in bytes.
const COMPILED_MOST: usize = 1 << 20;

/// The longest parameter or header name, in bytes.
const NAME_MOST: usize = 64;

/// The group a byte path names its item with.
const ITEM_NAME: &str = "item";

/// The group a live-stream path names its live stream with.
pub const STREAM_NAME: &str = "stream";

/// The statuses an answer can be.
const STATUS: std::ops::RangeInclusive<u16> = 100..=599;

/// The headers a guard may present a member's token in.
const SETTABLE: [&str; 4] = [
    "authorization",
    "x-api-key",
    "x-emby-token",
    "x-mediabrowser-token",
];

/// An address a declaration's paths and templates are read against, on no host anybody
/// holds.
const PROBE_BASE: &str = "https://household.invalid:8920";

/// What a probe puts where a template names the item.
const PROBE_ITEM: &str = "0123456789abcdef0123456789abcdef";

/// Why a declaration is refused at install.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Undeclarable {
    /// It serves bytes and declares no question to ask about them.
    #[error("it serves bytes under {pattern} and declares no question to ask about them")]
    Unasked {
        /// The byte path's pattern.
        pattern: String,
    },
    /// A byte path does not say where the item's id sits.
    #[error("its byte path {pattern} does not hold exactly one group named item")]
    Unplaced {
        /// The byte path's pattern.
        pattern: String,
    },
    /// A live-stream path does not say where the live stream's id sits.
    #[error("its live-stream path {pattern} does not hold exactly one group named stream")]
    Unstreamed {
        /// The live-stream path's pattern.
        pattern: String,
    },
    /// A pattern does not compile, is too large, or is not anchored where the guard
    /// relies on it.
    #[error("the pattern {pattern} is not one a guard can match with")]
    Unanchored {
        /// The pattern.
        pattern: String,
    },
    /// A template does not name the item.
    #[error("{template} does not name the item")]
    Unnamed {
        /// The template.
        template: String,
    },
    /// The question's path is not a plain path on the one it is sent to.
    #[error("the question's path {path} is not a plain path naming the item as a segment")]
    Unsent {
        /// The path.
        path: String,
    },
    /// A location is built on another host than the household address.
    #[error("the location {template} is not built on the household address")]
    Elsewhere {
        /// The template.
        template: String,
    },
    /// A parameter's name is not a plain name.
    #[error(
        "the parameter {name} is not named with letters, digits, underscores, dashes and dots"
    )]
    Unspelled {
        /// The name.
        name: String,
    },
    /// A token is presented in a header a guard may not write, or in a value that is not
    /// one line naming it once.
    #[error("a token presented as {header} could reach somewhere other than the upstream")]
    Unpresentable {
        /// The header's name.
        header: String,
    },
    /// The question's answers are not statuses, or one means two things.
    #[error("the question's answers overlap or are not statuses")]
    Unanswerable,
}

impl Declared {
    /// The declaration, where the core may hand it to a guard.
    ///
    /// # Errors
    ///
    /// [`Undeclarable`] for the first part of it a guard could not enforce, that could
    /// send a request or a member's token anywhere but the upstream or the adapter, or
    /// that would locate an item somewhere other than at the household address.
    pub fn checked(self) -> Result<Self, Undeclarable> {
        for path in &self.bytes {
            byte_path(path)?;
        }
        for pattern in &self.refused {
            path_pattern(pattern)?;
        }
        for path in &self.live {
            live_path(path)?;
        }
        match (&self.question, self.bytes.first()) {
            (None, Some(path)) => {
                return Err(Undeclarable::Unasked {
                    pattern: path.pattern.clone(),
                })
            }
            (Some(question), _) => asked(question)?,
            (None, None) => {}
        }
        token(&self.token)?;
        located(&self.locations)?;
        Ok(self)
    }
}

/// `pattern` parsed and compiled as a guard compiles it, where it is within the bounds.
fn compiled(pattern: &str) -> Result<(regex::Regex, regex_syntax::hir::Hir), Undeclarable> {
    let refused = || Undeclarable::Unanchored {
        pattern: pattern.to_owned(),
    };
    if pattern.len() > PATTERN_MOST {
        return Err(refused());
    }
    let parsed = regex_syntax::Parser::new()
        .parse(pattern)
        .map_err(|_| refused())?;
    let engine = regex::RegexBuilder::new(pattern)
        .size_limit(COMPILED_MOST)
        .dfa_size_limit(COMPILED_MOST)
        .build()
        .map_err(|_| refused())?;
    Ok((engine, parsed))
}

/// Whether `pattern` is a path pattern every match of which starts at the path's start.
fn path_pattern(pattern: &str) -> Result<regex::Regex, Undeclarable> {
    let (engine, parsed) = compiled(pattern)?;
    if parsed.properties().look_set_prefix().contains(Look::Start) {
        Ok(engine)
    } else {
        Err(Undeclarable::Unanchored {
            pattern: pattern.to_owned(),
        })
    }
}

/// Whether `shape` is one every match of which spans the whole value.
fn shape(shape: &str) -> Result<(), Undeclarable> {
    let (_, parsed) = compiled(shape)?;
    let looks = parsed.properties();
    let whole = looks.look_set_prefix().contains(Look::Start)
        && looks.look_set_suffix().contains(Look::End);
    if whole {
        Ok(())
    } else {
        Err(Undeclarable::Unanchored {
            pattern: shape.to_owned(),
        })
    }
}

/// Whether a byte path says once where its item sits, and is anchored throughout.
fn byte_path(path: &BytePath) -> Result<(), Undeclarable> {
    let engine = path_pattern(&path.pattern)?;
    let items = engine
        .capture_names()
        .filter(|name| *name == Some(ITEM_NAME))
        .count();
    if items != 1 {
        return Err(Undeclarable::Unplaced {
            pattern: path.pattern.clone(),
        });
    }
    shape(&path.item)?;
    for exempt in &path.exempt {
        path_pattern(exempt)?;
    }
    for named in &path.naming {
        plain(&named.parameter)?;
        if let Held::Asked { shape: asked } = &named.held {
            shape(asked)?;
        }
    }
    Ok(())
}

/// Whether a live-stream path says once where its live stream sits, and is anchored
/// throughout.
fn live_path(path: &LivePath) -> Result<(), Undeclarable> {
    let engine = path_pattern(&path.pattern)?;
    let streams = engine
        .capture_names()
        .filter(|name| *name == Some(STREAM_NAME))
        .count();
    if streams != 1 {
        return Err(Undeclarable::Unstreamed {
            pattern: path.pattern.clone(),
        });
    }
    shape(&path.stream)
}

/// Whether `name` is a plain parameter name.
fn plain(name: &str) -> Result<(), Undeclarable> {
    let spelled = (1..=NAME_MOST).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    if spelled {
        Ok(())
    } else {
        Err(Undeclarable::Unspelled {
            name: name.to_owned(),
        })
    }
}

/// Whether the question is a plain path naming the item as a whole segment, read the way
/// the guard reads it, and its answers mean one thing each.
fn asked(question: &Question) -> Result<(), Undeclarable> {
    let unsent = || Undeclarable::Unsent {
        path: question.path.clone(),
    };
    let rest = question.path.strip_prefix('/').ok_or_else(unsent)?;
    let mut named = false;
    for segment in rest.split('/') {
        if segment == ITEM {
            named = true;
            continue;
        }
        let plain_segment = !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
            });
        if !plain_segment {
            return Err(unsent());
        }
    }
    if !named {
        return Err(Undeclarable::Unnamed {
            template: question.path.clone(),
        });
    }
    joined(PROBE_BASE, &question.path.replace(ITEM, PROBE_ITEM)).ok_or_else(unsent)?;
    answered(&question.answers)
}

/// Whether every answer is a status and none means two things.
fn answered(answers: &Answers) -> Result<(), Undeclarable> {
    let ranges: Vec<&Statuses> = [
        &answers.may,
        &answers.may_not,
        &answers.no_session,
        &answers.not_from_there,
    ]
    .into_iter()
    .flatten()
    .collect();
    let valid = ranges.iter().all(|range| {
        STATUS.contains(&range.from) && STATUS.contains(&range.to) && range.from <= range.to
    });
    let overlapping = ranges.iter().enumerate().any(|(at, one)| {
        ranges
            .iter()
            .skip(at + 1)
            .any(|other| one.from <= other.to && other.from <= one.to)
    });
    if valid && !overlapping {
        Ok(())
    } else {
        Err(Undeclarable::Unanswerable)
    }
}

/// Whether the token reaches the upstream in one header a guard may set, or one plain
/// query parameter, and is held to a shape spanning the whole token.
fn token(token: &Token) -> Result<(), Undeclarable> {
    shape(&token.shape)?;
    for parameter in &token.parameters {
        plain(parameter)?;
    }
    match &token.presented {
        Presentation::AsIs => Ok(()),
        Presentation::Query { parameter } => plain(parameter),
        Presentation::Header { name, value } => {
            let settable = SETTABLE
                .iter()
                .any(|settable| name.eq_ignore_ascii_case(settable));
            let rest = value.replacen(TOKEN, "", 1);
            let one_line = value.matches(TOKEN).count() == 1
                && rest
                    .bytes()
                    .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b'{' | b'}'));
            if settable && one_line {
                Ok(())
            } else {
                Err(Undeclarable::Unpresentable {
                    header: name.clone(),
                })
            }
        }
    }
}

/// Whether every location is built on the household address, names the item, and holds
/// no placeholder but the two a location may.
fn located(locations: &Locations) -> Result<(), Undeclarable> {
    for template in [&locations.stream, &locations.poster, &locations.backdrop] {
        let elsewhere = || Undeclarable::Elsewhere {
            template: template.clone(),
        };
        let rest = template.strip_prefix(BASE).ok_or_else(elsewhere)?;
        if !rest.contains(ITEM) {
            return Err(Undeclarable::Unnamed {
                template: template.clone(),
            });
        }
        let filled = rest.replace(ITEM, PROBE_ITEM);
        if filled.contains(['{', '}']) {
            return Err(elsewhere());
        }
        Locations::located(template, PROBE_BASE, PROBE_ITEM).ok_or_else(elsewhere)?;
    }
    Ok(())
}

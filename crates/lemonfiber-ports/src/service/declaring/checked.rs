//! Whether a declaration is one a guard can enforce: nothing in it sends a request, or a
//! member's token, anywhere but the upstream or the adapter, and every location it gives
//! a member is on the household address.

use super::{
    Answers, BytePath, Declared, Held, Locations, Presentation, Question, Statuses, Token, BASE,
    ITEM, ITEM_GROUP, TOKEN,
};

/// The flag a pattern may open with: matched without regard to case.
const CASELESS: &str = "(?i)";

/// The statuses an answer can be.
const STATUS: std::ops::RangeInclusive<u16> = 100..=599;

/// The headers a presentation may not write: the request's own host, the ones a hop
/// consumes, its framing, and the ones that say which client it came from.
const UNWRITABLE: [&str; 15] = [
    "host",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
];

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
    /// A path pattern is not anchored at the path's start, or a shape at both ends.
    #[error("{pattern} is not anchored")]
    Unanchored {
        /// The pattern.
        pattern: String,
    },
    /// A template does not name the item.
    #[error("{template} does not name the item as {ITEM}")]
    Unnamed {
        /// The template.
        template: String,
    },
    /// The question's path is not a path on the one it is sent to.
    #[error("the question's path {path} is not a plain path naming the item as a segment")]
    Unsent {
        /// The path.
        path: String,
    },
    /// A location is built on another host than the household address.
    #[error("the location {template} is not built on {BASE}")]
    Elsewhere {
        /// The template.
        template: String,
    },
    /// A parameter's name is not a plain name.
    #[error("the parameter {name} is not named with ASCII letters, digits, `_`, `-` and `.`")]
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
            anchored_at_start(pattern)?;
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

/// Whether a byte path says where its item sits, and is anchored throughout.
fn byte_path(path: &BytePath) -> Result<(), Undeclarable> {
    if path.pattern.matches(ITEM_GROUP).count() != 1 {
        return Err(Undeclarable::Unplaced {
            pattern: path.pattern.clone(),
        });
    }
    anchored_at_start(&path.pattern)?;
    anchored_at_both_ends(&path.item)?;
    for exempt in &path.exempt {
        anchored_at_start(exempt)?;
    }
    for named in &path.naming {
        plain(&named.parameter)?;
        if let Held::Asked { shape } = &named.held {
            anchored_at_both_ends(shape)?;
        }
    }
    Ok(())
}

/// `pattern` without the flag it may open with.
fn flagless(pattern: &str) -> &str {
    pattern.strip_prefix(CASELESS).unwrap_or(pattern)
}

/// Whether `pattern` is anchored at the start of the path it is matched against.
fn anchored_at_start(pattern: &str) -> Result<(), Undeclarable> {
    if flagless(pattern).starts_with('^') {
        Ok(())
    } else {
        Err(Undeclarable::Unanchored {
            pattern: pattern.to_owned(),
        })
    }
}

/// Whether `shape` is anchored at both ends of the value it is matched against.
fn anchored_at_both_ends(shape: &str) -> Result<(), Undeclarable> {
    let bare = flagless(shape);
    let ended = bare.ends_with('$') && !bare.ends_with("\\$");
    if bare.starts_with('^') && ended {
        Ok(())
    } else {
        Err(Undeclarable::Unanchored {
            pattern: shape.to_owned(),
        })
    }
}

/// Whether `name` is a plain parameter name.
fn plain(name: &str) -> Result<(), Undeclarable> {
    let spelled = !name.is_empty()
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

/// Whether the question is a plain path naming the item as a whole segment, and its
/// answers mean one thing each.
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
    answered(&question.answers)
}

/// Whether every answer is a status and none means two things.
fn answered(answers: &Answers) -> Result<(), Undeclarable> {
    let sets = [
        &answers.may,
        &answers.may_not,
        &answers.no_session,
        &answers.not_from_there,
    ];
    let ranges: Vec<(usize, &Statuses)> = sets
        .iter()
        .enumerate()
        .flat_map(|(set, ranges)| ranges.iter().map(move |range| (set, range)))
        .collect();
    let valid = ranges.iter().all(|(_, range)| {
        STATUS.contains(&range.from) && STATUS.contains(&range.to) && range.from <= range.to
    });
    let overlapping = ranges.iter().enumerate().any(|(at, (_, one))| {
        ranges
            .iter()
            .skip(at + 1)
            .any(|(_, other)| one.from <= other.to && other.from <= one.to)
    });
    if valid && !overlapping {
        Ok(())
    } else {
        Err(Undeclarable::Unanswerable)
    }
}

/// Whether the token reaches the upstream in one header it may write, or one plain
/// query parameter, and is held to an anchored shape.
fn token(token: &Token) -> Result<(), Undeclarable> {
    anchored_at_both_ends(&token.shape)?;
    for parameter in &token.parameters {
        plain(parameter)?;
    }
    match &token.presented {
        Presentation::AsIs => Ok(()),
        Presentation::Query { parameter } => plain(parameter),
        Presentation::Header { name, value } => {
            let named = !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !UNWRITABLE
                    .iter()
                    .any(|unwritable| name.eq_ignore_ascii_case(unwritable));
            let one_line = value.bytes().all(|byte| (b' '..=b'~').contains(&byte));
            if named && one_line && value.matches(TOKEN).count() == 1 {
                Ok(())
            } else {
                Err(Undeclarable::Unpresentable {
                    header: name.clone(),
                })
            }
        }
    }
}

/// Whether every location is built on the household address and names the item.
fn located(locations: &Locations) -> Result<(), Undeclarable> {
    for template in [&locations.stream, &locations.poster, &locations.backdrop] {
        let on_base = template.strip_prefix(BASE).is_some_and(|rest| {
            rest.starts_with('/')
                && !rest.starts_with("//")
                && !rest.contains(BASE)
                && rest
                    .bytes()
                    .all(|byte| (b'!'..=b'~').contains(&byte) && byte != b'\\')
        });
        if !on_base {
            return Err(Undeclarable::Elsewhere {
                template: template.clone(),
            });
        }
        if !template.contains(ITEM) {
            return Err(Undeclarable::Unnamed {
                template: template.clone(),
            });
        }
    }
    Ok(())
}

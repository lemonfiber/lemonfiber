//! Where in an answer an expectation is looking.
//!
//! An expectation's key used to be the name of a top-level member and nothing else,
//! which is a rule that holds for exactly as long as every service answers with a flat
//! object. Plex nests every response one level under `MediaContainer`, and the setting
//! that says whether it has published itself to the internet lives in an array of a
//! hundred and fifty-one, found by the `id` one of them carries. Neither is reachable
//! by a name.
//!
//! So a key is a place rather than a name, and the place is written as a **JSON Pointer**
//! — RFC 6901, unchanged — with one addition of this project's own: a step that picks
//! the one entry of an array by a field it holds. The address of the RFC is in the
//! contract rather than here, because a host named in shipped source is a host somebody
//! has to answer for, and nothing in this file asks anything of anybody.
//!
//! **A key that does not begin with `/` is still the name of a top-level member**, which
//! is what every key written before this generation is, and is why none of them changed
//! meaning. RFC 6901 gives the empty pointer to the whole document; here the empty string
//! is the member named `""`, because a key is a name until it says otherwise.
//!
//! The addition is one step and one comparison. A reference token written `[field=value]`
//! means *the entry of this array whose `field` holds `value`*, exactly one of them must,
//! and there are no operators, no wildcards and no indices. An index would be the trap
//! the selector exists to avoid: the order of Plex's settings is not a promise anybody
//! made, so the ninety-first entry is the wrong answer one release later.
//!
//! Its cost is stated rather than hidden: `[` and `]` belong to the selector, so a
//! reference token carrying either is refused rather than read as a member name. A
//! member actually called `[a=b]` is therefore unreachable through a pointer — nothing
//! in the bundled stack, either published plugin, or Plex has one, and the alternative
//! is a key whose meaning depends on the answer it is read against.

/// One step of the way to what an expectation is looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A member of an object, by name.
    Named(String),
    /// The one entry of an array whose field holds a given value.
    Selected {
        /// The field each entry is asked for.
        field: String,
        /// What that field must hold, as the key writes it.
        value: String,
    },
}

/// Why a key names no place.
///
/// Carried as the sentence an author reads rather than as a code, because there is one
/// caller and what it does with this is put it in front of whoever wrote the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

impl std::fmt::Display for Malformed {
    fn fmt(&self, into: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(into, "{}", self.0)
    }
}

/// The steps a key names, or why it names none.
///
/// # Errors
///
/// [`Malformed`] where a key begins as a pointer and is not one: an escape RFC 6901 does
/// not define, a selector missing its field or its value, or a reference token carrying
/// a bracket that is not a selector's.
pub fn steps(key: &str) -> Result<Vec<Step>, Malformed> {
    let Some(rest) = key.strip_prefix('/') else {
        return Ok(vec![Step::Named(key.to_owned())]);
    };
    rest.split('/').map(token).collect()
}

/// One reference token, as a step.
fn token(token: &str) -> Result<Step, Malformed> {
    if let Some(inside) = token.strip_prefix('[').and_then(|it| it.strip_suffix(']')) {
        return selected(inside, token);
    }
    if token.contains('[') || token.contains(']') {
        return Err(Malformed(format!(
            "`{token}` carries a bracket, and `[` and `]` are the selector's; an entry of a list \
             is picked by a step of its own, written `[field=value]`"
        )));
    }
    unescaped(token).map(Step::Named)
}

/// One selector, from what is between its brackets.
fn selected(inside: &str, token: &str) -> Result<Step, Malformed> {
    let Some((field, value)) = inside.split_once('=') else {
        return Err(Malformed(format!(
            "`{token}` names no field to pick by; a selector is written `[field=value]`"
        )));
    };
    if field.is_empty() {
        return Err(Malformed(format!(
            "`{token}` picks by no field; a selector is written `[field=value]`"
        )));
    }
    if field.contains('[') || field.contains(']') || value.contains('[') || value.contains(']') {
        return Err(Malformed(format!(
            "`{token}` carries a bracket inside a selector, and one selector picks one entry"
        )));
    }
    Ok(Step::Selected {
        field: unescaped(field)?,
        value: unescaped(value)?,
    })
}

/// A reference token with RFC 6901's two escapes read.
///
/// `~1` is a `/` and `~0` is a `~`, in that order, because reading them the other way
/// round turns `~01` into a `/` where the RFC says it is `~1`. Done in one pass rather
/// than by two replacements for exactly that reason.
///
/// A `~` followed by anything else is refused rather than passed through. RFC 6901 does
/// not define it, so the two readings — a literal tilde, or an escape somebody mistyped
/// — cannot be told apart, and a key that quietly meant the first would look for a
/// member nobody has.
fn unescaped(token: &str) -> Result<String, Malformed> {
    let mut read = String::with_capacity(token.len());
    let mut letters = token.chars();
    while let Some(letter) = letters.next() {
        if letter != '~' {
            read.push(letter);
            continue;
        }
        match letters.next() {
            Some('0') => read.push('~'),
            Some('1') => read.push('/'),
            Some(next) => {
                return Err(Malformed(format!(
                    "`{token}` carries `~{next}`, and the only escapes are `~0` for a tilde and \
                     `~1` for a slash"
                )))
            }
            None => {
                return Err(Malformed(format!(
                    "`{token}` ends in a `~`, which begins an escape and finishes none"
                )))
            }
        }
    }
    Ok(read)
}

/// The steps, written back as the key that named them.
///
/// Used by a refusal to say how far it got, so the part of a key that resolved is shown
/// in the same spelling the author wrote — an author holding a manifest and a refusal
/// should not have to translate between two ways of writing one place.
#[must_use]
pub fn said(steps: &[Step]) -> String {
    let mut written = String::new();
    for step in steps {
        written.push('/');
        match step {
            Step::Named(name) => written.push_str(&escaped(name)),
            Step::Selected { field, value } => {
                written.push('[');
                written.push_str(&escaped(field));
                written.push('=');
                written.push_str(&escaped(value));
                written.push(']');
            }
        }
    }
    written
}

/// A name with RFC 6901's two escapes written back in.
fn escaped(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests;

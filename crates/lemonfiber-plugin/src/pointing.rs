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
mod tests {
    use serde_json::json;

    use super::{said, steps, Step};

    /// The steps a key names, or the sentence it was refused with.
    fn read(key: &str) -> Result<Vec<Step>, String> {
        steps(key).map_err(|why| why.0)
    }

    /// A name, which is every key written before pointers existed.
    #[test]
    fn a_key_that_is_not_a_pointer_is_the_name_of_a_top_level_member() {
        assert_eq!(read("content"), Ok(vec![Step::Named("content".to_owned())]));
        // A dot is part of the name and not a separator, which is what makes this
        // generation's reading of an old key the same as the last one's.
        assert_eq!(
            read("MediaContainer.size"),
            Ok(vec![Step::Named("MediaContainer.size".to_owned())])
        );
        // The empty string is the member named "", not the whole document: a key is a
        // name until it begins with a slash.
        assert_eq!(read(""), Ok(vec![Step::Named(String::new())]));
        // A bracket in a name is a name. The selector's rule is a pointer's rule.
        assert_eq!(read("a[0]"), Ok(vec![Step::Named("a[0]".to_owned())]));
    }

    #[test]
    fn a_pointer_names_each_member_on_the_way_down() {
        assert_eq!(
            read("/MediaContainer/machineIdentifier"),
            Ok(vec![
                Step::Named("MediaContainer".to_owned()),
                Step::Named("machineIdentifier".to_owned()),
            ])
        );
        // One step, which is the same place a bare name reaches and says so differently.
        assert_eq!(
            read("/content"),
            Ok(vec![Step::Named("content".to_owned())])
        );
        // RFC 6901's own case: a pointer of one empty token is the member named "".
        assert_eq!(read("/"), Ok(vec![Step::Named(String::new())]));
    }

    #[test]
    fn a_selector_picks_an_entry_of_a_list_by_a_field_it_holds() {
        assert_eq!(
            read("/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value"),
            Ok(vec![
                Step::Named("MediaContainer".to_owned()),
                Step::Named("Setting".to_owned()),
                Step::Selected {
                    field: "id".to_owned(),
                    value: "PublishServerOnPlexOnlineKey".to_owned(),
                },
                Step::Named("value".to_owned()),
            ])
        );
    }

    /// A value may be empty, and a field may not.
    ///
    /// Plex answers `"group": ""` for a hundred of its settings, so *the one whose group
    /// is nothing* is a thing somebody will want to write. A field that is nothing is not
    /// the same: there is no member to ask each entry for.
    #[test]
    fn a_selector_may_pick_by_a_field_holding_nothing_and_may_not_pick_by_no_field() {
        assert_eq!(
            read("/[group=]"),
            Ok(vec![Step::Selected {
                field: "group".to_owned(),
                value: String::new(),
            }])
        );
        let said = read("/[=x]").err().unwrap_or_default();
        assert!(said.contains("picks by no field"), "got: {said}");
        assert!(said.contains("[=x]"), "names the token: {said}");
    }

    /// A selector needs both halves, and the refusal shows the shape it wanted.
    #[test]
    fn a_selector_with_nothing_to_compare_is_refused_by_name() {
        let said = read("/Setting/[id]").err().unwrap_or_default();
        assert!(said.contains("names no field to pick by"), "got: {said}");
        assert!(said.contains("[field=value]"), "shows the shape: {said}");
        assert!(said.contains("[id]"), "names the token: {said}");
    }

    /// The suffix form the issue and the draft manifest both reach for.
    ///
    /// `Setting[id=X]` reads as a member actually called that, which is a key nothing
    /// answers and a mistake nothing would report — so a bracket in a reference token is
    /// refused, and the refusal shows where the selector goes instead.
    #[test]
    fn a_selector_written_onto_a_member_name_is_refused_and_told_where_it_goes() {
        let said = read("/MediaContainer/Setting[id=X]/value")
            .err()
            .unwrap_or_default();
        assert!(said.contains("carries a bracket"), "got: {said}");
        assert!(said.contains("[field=value]"), "shows the shape: {said}");
        assert!(said.contains("Setting[id=X]"), "names the token: {said}");
        // The closing half alone is the same mistake and is refused the same way.
        assert!(read("/Setting]").is_err(), "a stray bracket is not a name");
    }

    /// A selector inside a selector is one step trying to be two.
    #[test]
    fn a_bracket_inside_a_selector_is_refused() {
        let said = read("/[id=[x]]").err().unwrap_or_default();
        assert!(said.contains("inside a selector"), "got: {said}");
    }

    /// RFC 6901's escapes, and the order they are read in.
    ///
    /// `~01` is `~1` and not `/`, which is the case the two-replacement reading gets
    /// wrong — and `~1~0` is `/~`, which is the case that shows the pass is doing both.
    #[test]
    fn the_two_escapes_are_read_in_one_pass_and_in_the_right_order() {
        assert_eq!(read("/a~1b"), Ok(vec![Step::Named("a/b".to_owned())]));
        assert_eq!(read("/a~0b"), Ok(vec![Step::Named("a~b".to_owned())]));
        assert_eq!(read("/~01"), Ok(vec![Step::Named("~1".to_owned())]));
        assert_eq!(read("/~1~0"), Ok(vec![Step::Named("/~".to_owned())]));
        // Both halves of a selector are read the same way, or a field with a slash in it
        // would be reachable in a member name and not in a selector.
        assert_eq!(
            read("/[a~1b=c~0d]"),
            Ok(vec![Step::Selected {
                field: "a/b".to_owned(),
                value: "c~d".to_owned(),
            }])
        );
    }

    /// An escape the RFC does not define is refused rather than read as a tilde.
    #[test]
    fn an_escape_that_is_not_one_is_refused_by_name() {
        let said = read("/a~2b").err().unwrap_or_default();
        assert!(said.contains("`~2`"), "names what it carries: {said}");
        assert!(said.contains("~0"), "names the escapes: {said}");
        let ends = read("/a~").err().unwrap_or_default();
        assert!(ends.contains("ends in a `~`"), "got: {ends}");
        // Inside a selector too, on either side of the comparison.
        assert!(read("/[a~2=b]").is_err(), "a field's escape is read");
        assert!(read("/[a=b~2]").is_err(), "a value's escape is read");
    }

    /// Every pointer without a selector reaches the place `serde_json` reaches.
    ///
    /// The standard half of this is RFC 6901's and is not ours to re-specify, so it is
    /// held to an implementation of the RFC that is already in the tree rather than to a
    /// second reading of the document. Only the selector is this project's.
    #[test]
    fn the_standard_half_agrees_with_the_pointer_reader_already_here() {
        let document = json!({
            "": {"empty": 1},
            "a/b": 2,
            "a~b": 3,
            "MediaContainer": {"size": 0, "Setting": [{"id": "x", "value": false}]},
            "list": [10, 20],
        });
        for pointer in [
            "/MediaContainer",
            "/MediaContainer/size",
            "/MediaContainer/Setting",
            "/a~1b",
            "/a~0b",
            "//empty",
            "/nowhere",
            "/MediaContainer/nowhere",
            "/list",
        ] {
            let theirs = document.pointer(pointer);
            let ours = steps(pointer)
                .ok()
                .and_then(|steps| walked(&document, &steps));
            assert_eq!(ours, theirs, "{pointer}");
        }
    }

    /// The document at a place, for the agreement test above and nothing else.
    ///
    /// Deliberately not the resolver a verdict is reached with: that one reports where it
    /// stopped, and comparing two resolvers wants the plainer shape.
    fn walked<'a>(
        document: &'a serde_json::Value,
        steps: &[Step],
    ) -> Option<&'a serde_json::Value> {
        let mut at = document;
        for step in steps {
            match step {
                Step::Named(name) => at = at.as_object()?.get(name)?,
                Step::Selected { .. } => return None,
            }
        }
        Some(at)
    }

    /// A key is written back as the author wrote it, so a refusal and a manifest agree.
    #[test]
    fn steps_are_written_back_as_the_key_that_named_them() {
        for key in [
            "/MediaContainer/machineIdentifier",
            "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value",
            "/a~1b/[c~0d=e]",
            "/",
        ] {
            let read = steps(key).unwrap_or_default();
            assert_eq!(said(&read), key, "{key}");
        }
        // A plain name is one step, and writing it back gives the pointer that names the
        // same member rather than the name itself — which is what a refusal should show
        // for a partial walk, and is why this is not the identity.
        assert_eq!(said(&[Step::Named("content".to_owned())]), "/content");
    }
}

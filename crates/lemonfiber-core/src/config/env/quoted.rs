//! A value as the environment file spells it, and the value it spells.
//!
//! Compose reads this file and expands `$NAME` and `${NAME}` in every value that is not
//! single-quoted. Several values here are read out of files the containers write about
//! themselves, so a value written as it came would let one container have Compose fill
//! its own setting with another's secret. A value is therefore written so Compose reads
//! back exactly that value, and read the way Compose reads it, so lemonfiber and the stack
//! never hold two different ideas of one setting.
//!
//! The reading follows Compose's own. A value in double quotes takes the backslash
//! escapes `\\`, `\"` and `\$` and the control characters `\a`, `\b`, `\f`, `\n`, `\r`,
//! `\t` and `\v`, and `$$` stands for one `$`. A value in single quotes is taken as
//! written apart from `\'`. An unquoted value ends at ` #` and loses the space after it,
//! and `$$` stands for one `$` there too. A `${NAME}` lemonfiber did not write is the
//! operator's, and is read as written: the expansion is Compose's to do.

/// Whether a value can be written as it is, with nothing in it Compose reads as anything
/// but itself.
fn plain(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"_-./:@+,=%~".contains(&byte))
}

/// The value as it is written into the file: as it is where it is plain, and otherwise in
/// double quotes with every `\`, `"` and `$` escaped, which Compose reads back as exactly
/// this value and expands nothing in.
pub(super) fn written(value: &str) -> String {
    if plain(value) {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        if matches!(character, '\\' | '"' | '$') {
            out.push('\\');
        }
        out.push(character);
    }
    out.push('"');
    out
}

/// The value a line's text after its `=` spells, as Compose reads it.
pub(super) fn read(raw: &str) -> String {
    let text = raw.trim_start();
    if let Some(quote @ ('"' | '\'')) = text.chars().next() {
        return match quoted(text, quote) {
            Some(inside) if quote == '"' => dollars(&escapes(&inside)),
            Some(inside) => inside,
            // An opening quote nothing closes is a value Compose refuses outright; the
            // text is kept as written, which is what the operator will find there.
            None => text.to_owned(),
        };
    }
    let value = text.split_once(" #").map_or(text, |(value, _)| value);
    dollars(value.trim_end())
}

/// What lies between the opening quote and the one that closes it, with that quote
/// unescaped where a `\` stood before it and every other `\` left for the escapes.
fn quoted(text: &str, quote: char) -> Option<String> {
    let mut inside = String::new();
    let mut escaping = false;
    for character in text.chars().skip(1) {
        if character == quote && !escaping {
            return Some(inside);
        }
        if escaping {
            escaping = false;
            if character != quote {
                inside.push('\\');
            }
            inside.push(character);
        } else if character == '\\' {
            escaping = true;
        } else {
            inside.push(character);
        }
    }
    None
}

/// The escapes a double-quoted value takes, with `\$` kept as the `$$` that stands for
/// one `$` once [`dollars`] has read it.
fn escapes(inside: &str) -> String {
    let mut out = String::with_capacity(inside.len());
    let mut characters = inside.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        // What [`quoted`] hands over never ends on a lone `\`: one there escaped the
        // closing quote, which then did not close it. Read as `\\` all the same. Nor
        // does it hold `\"`, which it has already read as the quote alone.
        match characters.next().unwrap_or('\\') {
            '\\' => out.push('\\'),
            '$' => out.push_str("$$"),
            'a' => out.push('\u{7}'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\u{b}'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// Every `$$` read as the one `$` it stands for, from the left.
fn dollars(value: &str) -> String {
    value.replace("$$", "$")
}

#[cfg(test)]
mod tests;

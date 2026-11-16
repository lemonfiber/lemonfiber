//! What an ask came to, or would come to, said as a household says it.

use crate::model::{Became, LeftAfter};

/// What an ask came to, or would come to, in one household sentence.
pub(in crate::wishing) fn said(
    outcome: Became,
    released: bool,
    leaves: &LeftAfter,
    rehearsed: bool,
) -> String {
    let mut said = match (outcome, rehearsed) {
        (Became::Waiting, true) => {
            "Asking for this waits for whoever runs the house to say yes.".to_owned()
        }
        (_, true) => "Asking for this brings it without anybody saying yes first.".to_owned(),
        (Became::Waiting, false) => "Asked. Whoever runs the house will say yes or no.".to_owned(),
        (_, false) => "Asked. It's on its way.".to_owned(),
    };
    if !released {
        said.push_str(" It isn't out yet, so it arrives once it is.");
    }
    if let Some(room) = room(leaves) {
        said.push(' ');
        said.push_str(&room);
    }
    said
}

/// What is left afterwards, in titles of each kind and never as a size; nothing where no
/// limit holds the member.
fn room(leaves: &LeftAfter) -> Option<String> {
    let films = leaves.films.map(|films| more(films, "film", ""));
    let television = leaves
        .television
        .map(|seasons| more(seasons, "season", " of television"));
    let music = leaves.music.map(|asks| more(asks, "ask", " for music"));
    let books = leaves.books.map(|asks| more(asks, "ask", " for books"));
    let parts: Vec<String> = films
        .into_iter()
        .chain(television)
        .chain(music)
        .chain(books)
        .collect();
    (!parts.is_empty()).then(|| format!("After this, there's room for {}.", parts.join(" and ")))
}

/// `count` more of `noun`, as a household says it: none is "no more".
fn more(count: u32, noun: &str, after: &str) -> String {
    let many = if count == 0 {
        "no".to_owned()
    } else {
        count.to_string()
    };
    format!("{many} more {noun}{}{after}", plural(count))
}

/// The ending a count of `many` takes.
fn plural(many: u32) -> &'static str {
    crate::plural::s(usize::try_from(many).unwrap_or(usize::MAX))
}

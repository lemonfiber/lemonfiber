//! Where this product listens, and what could move it.
//!
//! Three things hold the web surface on loopback, and only the first is a line
//! somebody could read and believe: the address is a loopback constant; nothing
//! else in the shipped half of this workspace takes a socket at all; and nothing an
//! operator types reaches the address. A guard over the first alone would restate
//! the function it was reading — the function looks the same whether or not a flag
//! somewhere else can override it.
//!
//! **The corpus is what ships**, which is narrower than what is under a `src/` in
//! two ways. A file's own tests are cut off the bottom, and a module the compiler
//! only builds for tests is dropped whole — five modules here are declared that
//! way, to keep a fake beside the code it fakes. A test that binds a socket is a
//! test: it serves nobody, and holding the fake servers in this workspace to this
//! rule would put every one of them on an exception list and teach whoever reads it
//! that the list means nothing.
//!
//! **What is not covered**, since a guard that reads as wider than it is becomes a
//! reason not to look: an address arriving as a hostname resolved when the process
//! runs, and one assembled from parts rather than written down. Neither exists
//! here; both would be a change to the one function that makes the address, in the
//! one file this names.

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};

mod source_tree;

use source_tree::shipped;

/// The calls that take an address from the operating system, and the call that
/// serves on what one of them gave.
///
/// Written as the call rather than as the type, so importing `TcpListener` to name
/// it in a signature is not read as listening on one.
const LISTENING: &[&str] = &[
    "TcpListener::bind",
    "TcpSocket::bind",
    "UdpSocket::bind",
    "UnixListener::bind",
    "axum::serve(",
];

/// The named address constant that is loopback.
const LOOPBACK: &str = "LOCALHOST";

/// The files that listen, and what each one serves.
///
/// One entry, and the single entry is the point. A second file arriving here is red
/// until somebody writes down what it serves, and writing that down is the moment
/// anybody asks what address it took.
const LISTENS: &[(&str, &str)] = &[(
    "crates/lemonfiber/src/ui.rs",
    "the web surface, the one thing this product serves and the reason this list is short",
)];

/// What the web surface can be asked for, and what each answer settles.
///
/// No address is on this list, and that is the whole of it. Loopback is not merely
/// the default here — there is nothing to type that would move it — and the
/// difference is invisible in the function that builds the address, which reads
/// exactly the same either way.
const ASKED: &[(&str, &str)] = &[
    (
        "port",
        "which port on this machine, and nothing about which machine or which interface",
    ),
    (
        "no_browser",
        "whether a desktop is asked to open one, which settles nothing about the socket",
    ),
    (
        "assets",
        "the directory the app being served is read from, which is a path and not an address",
    ),
    (
        "set_password",
        "whether a password is asked for before it starts, which settles who may open it and \
         nothing about where it is opened from",
    ),
];

/// An address named in the workspace: what was written, where, and what it reaches.
#[derive(Debug)]
struct Named {
    /// The file it was written in.
    file: String,
    /// The line it was written on, counted from one.
    line: usize,
    /// The address as it was written.
    written: String,
    /// Whether it names somewhere only this machine can reach.
    loopback: bool,
}

/// The text inside double quotes on one line.
fn quoted(line: &str) -> impl Iterator<Item = &str> {
    line.split('"').skip(1).step_by(2)
}

/// The address a literal is, where it is one.
///
/// A literal that parses is an address whoever typed it meant. One that does not is
/// a sentence, a path or another program's configuration file, and reading those
/// would report the prose rather than the product.
fn parsed(said: &str) -> Option<(String, IpAddr)> {
    let trimmed = said.trim();
    let address = trimmed
        .parse::<IpAddr>()
        .ok()
        .or_else(|| trimmed.parse::<SocketAddr>().ok().map(|socket| socket.ip()))?;
    Some((trimmed.to_owned(), address))
}

/// The constants named on `Ipv4Addr` or `Ipv6Addr` on one line.
///
/// The word after the colons, whatever it is. A method is taken as readily as a
/// constant, because an address built from four numbers in code that ships is the
/// same decision written less legibly.
fn constants(line: &str) -> impl Iterator<Item = String> + '_ {
    ["Ipv4Addr::", "Ipv6Addr::"]
        .into_iter()
        .flat_map(move |marker| line.match_indices(marker).zip(std::iter::repeat(marker)))
        .filter_map(|((at, _), marker)| line.get(at + marker.len()..))
        .map(|rest| {
            rest.chars()
                .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
                .collect()
        })
}

/// The addresses written on one line, each with whether it is loopback.
fn addresses_on(line: &str) -> Vec<(String, bool)> {
    let mut found: Vec<(String, bool)> = constants(line)
        .map(|named| {
            let loopback = named == LOOPBACK;
            (named, loopback)
        })
        .collect();
    found.extend(
        quoted(line)
            .filter_map(parsed)
            .map(|(written, address)| (written, address.is_loopback())),
    );
    found
}

/// Every address the shipped half of this workspace names.
fn named() -> Vec<Named> {
    let mut found = Vec::new();
    for (file, ships) in shipped() {
        for (number, line) in ships.lines().enumerate() {
            found.extend(
                addresses_on(line)
                    .into_iter()
                    .map(|(written, loopback)| Named {
                        file: file.clone(),
                        line: number + 1,
                        written,
                        loopback,
                    }),
            );
        }
    }
    found
}

/// The fields one struct declares.
///
/// Read from the text rather than from the type, because what is being checked is
/// the surface an operator types at. A field here is a flag, and a flag naming an
/// address is the defect this exists for.
fn fields_of<'a>(text: &'a str, name: &str) -> BTreeSet<&'a str> {
    let opening = format!("pub struct {name} {{");
    text.lines()
        .skip_while(|line| !line.starts_with(opening.as_str()))
        .skip(1)
        .take_while(|line| *line != "}")
        .filter_map(|line| line.trim_start().strip_prefix("pub "))
        .filter_map(|field| field.split(':').next())
        .collect()
}

/// Nothing that ships takes a socket without being written down.
///
/// The address a listener is given is checked below, and that check is worth
/// nothing on its own: it reads the files this workspace has today. This is the
/// half that survives a file being added — a new listener anywhere is red before
/// anybody has to notice it in review.
#[test]
fn nothing_that_ships_listens_without_being_written_down() {
    let declared: BTreeSet<String> = LISTENS.iter().map(|(file, _)| (*file).to_owned()).collect();
    let listening: BTreeSet<String> = shipped()
        .into_iter()
        .filter(|(_, ships)| LISTENING.iter().any(|call| ships.contains(call)))
        .map(|(file, _)| file)
        .collect();
    assert!(
        !listening.is_empty(),
        "nothing in this workspace listens, which means this is looking for the wrong call"
    );
    assert_eq!(
        listening, declared,
        "the shipped half of these files takes a socket and this list disagrees about which \
         may — add the file with what it serves, or serve it from the surface that already \
         listens"
    );
}

/// Every address the shipped half of this workspace names is loopback.
///
/// The surface behind it can start, stop and reconfigure the whole stack and
/// reaches every credential the system holds, so it is the one thing here that is
/// never offered to the network. Read from the words rather than from what a run
/// binds, because a run binds the address it was given and this is about the
/// addresses that exist to be given.
#[test]
fn every_address_that_ships_is_one_only_this_machine_reaches() {
    let found = named();
    assert!(
        found.iter().any(|address| address.file.ends_with("/ui.rs")),
        "the sweep found no address in the file that takes the socket, so it is reading the \
         wrong thing: {found:?}"
    );
    let beyond: Vec<String> = found
        .iter()
        .filter(|address| !address.loopback)
        .map(|address| format!("{}:{}: `{}`", address.file, address.line, address.written))
        .collect();
    assert!(
        beyond.is_empty(),
        "these name somewhere other than this machine, and something that ships can be given \
         one of them to listen on: {beyond:?}"
    );
}

/// The web surface can be asked for nothing that names where it listens.
///
/// The other half of the same property, and the half a reader cannot see: an
/// address built from a loopback constant is still an address an operator can move
/// if a flag reaches it. Held as the whole set rather than as a search for
/// address-sounding words, because the next flag will be called whatever it is
/// called, and a list of words to refuse would have to have guessed it.
#[test]
fn the_web_surface_can_be_asked_for_nothing_that_names_where_it_listens() {
    let declared: BTreeSet<&str> = ASKED.iter().map(|(flag, _)| *flag).collect();
    let Some(parsing) = shipped().get("crates/lemonfiber/src/cli.rs").cloned() else {
        unreachable!("the command line this binary parses lives in the tree this crawl reads")
    };
    assert_eq!(
        fields_of(&parsing, "RawUi"),
        declared,
        "the web surface takes flags this list does not name — say what each settles, and \
         say it having checked that none of them settles the address"
    );
}

/// Every declared entry says why it is one.
///
/// A list whose entries may be blank is a list of names, and a name explains
/// nothing to whoever reads it next. A reason is a sentence somebody can disagree
/// with.
#[test]
fn every_declared_entry_says_what_it_is_for() {
    let silent: Vec<&str> = LISTENS
        .iter()
        .chain(ASKED.iter())
        .filter(|(_, reason)| reason.split_whitespace().count() < 4)
        .map(|(named, _)| *named)
        .collect();
    assert!(
        silent.is_empty(),
        "these are written down as allowed and the list does not say what for: {silent:?}"
    );
}

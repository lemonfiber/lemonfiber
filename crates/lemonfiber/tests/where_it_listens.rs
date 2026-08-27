//! Where this product listens, and what could move it.
//!
//! The web surface answers this machine unless somebody asks otherwise, and asking
//! otherwise is refused without a password. Three things hold that, and only the
//! first is a line somebody could read and believe: every address beyond this
//! machine that the shipped half of this workspace names is one the policy names,
//! and is named in the one file that decides the tier; nothing else in that half
//! takes a socket at all; and the one thing an operator can type about it settles a
//! **tier** rather than an address. A guard over the first alone would restate the
//! function it was reading — the function looks the same whether or not a flag
//! somewhere else can override it.
//!
//! **Both families or neither.** The tier that answers this machine names an address
//! on IPv4 and one on IPv6, and so does the tier that answers a network. A policy
//! that named one family and not the other would be enforced on the one it named and
//! absent on the one it did not, which reads as enforced and is worse than nothing.
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
use std::fs;
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

/// What wrapping a socket in TLS would take, in the shapes it comes in.
///
/// Read from the manifest of the crate that takes the socket rather than from its
/// code, because a certificate turned on is a dependency first: the code that would
/// do it cannot be written until something is there to write it with, and a
/// dependency arriving is a line in a diff somebody has to add on purpose.
const TLS: &[&str] = &[
    "rustls",
    "native-tls",
    "tokio-native-tls",
    "openssl",
    "axum-server",
];

/// The files that listen, and what each one serves.
///
/// Two entries, and the pair is the point: one file decides which addresses may be
/// taken and takes them, and one holds what was taken open. A third arriving here is
/// red until somebody writes down what it serves, and writing that down is the moment
/// anybody asks what address it took.
const LISTENS: &[(&str, &str)] = &[
    (
        "crates/lemonfiber/src/ui/reach.rs",
        "the web surface's sockets, and the policy that decides which addresses it may \
         ask for at all",
    ),
    (
        "crates/lemonfiber/src/ui.rs",
        "holding open what that policy handed over, which is the other half of the one \
         thing this product serves",
    ),
];

/// The one file allowed to name an address beyond this machine.
///
/// Named as a path rather than as a rule about paths, because the point is that
/// there is exactly one of it: an address written anywhere else is one nothing had to
/// argue for.
const DECIDES: &str = "crates/lemonfiber/src/ui/reach.rs";

/// The addresses beyond this machine that the policy names, and what each is for.
///
/// Two, and they are one decision written on both families. An entry arriving here
/// is somebody writing down that a new address may be listened on, which is the
/// moment to ask what would then be reachable from where.
const POLICY: &[(&str, &str)] = &[
    (
        "Ipv4Addr::UNSPECIFIED",
        "every interface, which is the household tier: opt-in, and refused unless a \
         password has been set",
    ),
    (
        "Ipv6Addr::UNSPECIFIED",
        "the same tier on the other family, because a policy that named one and not the \
         other would be absent on the one it did not name",
    ),
];

/// What the web surface can be asked for, and what each answer settles.
///
/// No address is on this list, and that is the whole of it. One of them settles how
/// far it may be reached, and what it settles is a **tier**: two words, whose
/// addresses are the policy's to name and whose network half is refused without a
/// password. There is still nothing to type that names an address, and the difference
/// is invisible in the function that builds one, which reads exactly the same either
/// way.
const ASKED: &[(&str, &str)] = &[
    (
        "lan",
        "how far it may be reached, which is a tier and not an address — what a tier \
         comes to is the policy's, and what the policy allows is refused without a \
         password",
    ),
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

/// The constants named on `Ipv4Addr` or `Ipv6Addr` on one line, written whole.
///
/// The type and the word after the colons, because which family an address belongs
/// to is half of what this file is about — a policy is not applied equally to two
/// families by a reader that cannot tell them apart. A method is taken as readily as
/// a constant, because an address built from four numbers in code that ships is the
/// same decision written less legibly.
fn constants(line: &str) -> impl Iterator<Item = String> + '_ {
    ["Ipv4Addr::", "Ipv6Addr::"]
        .into_iter()
        .flat_map(move |marker| line.match_indices(marker).zip(std::iter::repeat(marker)))
        .filter_map(|((at, _), marker)| Some((marker, line.get(at + marker.len()..)?)))
        .map(|(marker, rest)| {
            let named: String = rest
                .chars()
                .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
                .collect();
            format!("{marker}{named}")
        })
}

/// The addresses written on one line, each with whether it is loopback.
fn addresses_on(line: &str) -> Vec<(String, bool)> {
    let mut found: Vec<(String, bool)> = constants(line)
        .map(|named| {
            let loopback = named.ends_with(LOOPBACK);
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

/// Every address beyond this machine is one the policy names, in the file that
/// decides the tier.
///
/// Loopback needs nothing written down: it names this machine, and no arrangement of
/// this machine's own addresses puts anything on a network. Everything else does,
/// and it has to be here — an address written somewhere else is one nothing had to
/// argue for, and the argument is the only thing standing between the most
/// privileged surface in the product and a network nobody chose.
///
/// Read from the words rather than from what a run binds, because a run binds the
/// address it was given and this is about the addresses that exist to be given.
#[test]
fn every_address_beyond_this_machine_is_one_the_policy_names() {
    let found = named();
    assert!(
        found
            .iter()
            .any(|address| address.file.ends_with("/ui/reach.rs")),
        "the sweep found no address in the file that decides the tier, so it is reading the \
         wrong thing: {found:?}"
    );
    let declared: BTreeSet<&str> = POLICY.iter().map(|(named, _)| *named).collect();
    let unnamed: Vec<String> = found
        .iter()
        .filter(|address| !address.loopback)
        .filter(|address| !declared.contains(address.written.as_str()) || address.file != DECIDES)
        .map(|address| format!("{}:{}: `{}`", address.file, address.line, address.written))
        .collect();
    assert!(
        unnamed.is_empty(),
        "these name somewhere other than this machine, and either the policy does not name \
         them or they are written outside {DECIDES}: {unnamed:?}"
    );
}

/// The policy names each tier on both families.
///
/// The failure this exists for is not an address that should not be there; it is one
/// that should and is not. A tier named on IPv4 alone is a rule enforced on IPv4
/// alone, which reads as enforced everywhere — and the operator who checks their
/// firewall against it is checking the half that was written down.
#[test]
fn the_policy_names_each_tier_on_both_families() {
    let deciding: Vec<String> = named()
        .into_iter()
        .filter(|address| address.file == DECIDES)
        .map(|address| address.written)
        .collect();
    assert!(
        !deciding.is_empty(),
        "nothing in {DECIDES} names an address, so this is reading the wrong file"
    );
    for tier in ["LOCALHOST", "UNSPECIFIED"] {
        for family in ["Ipv4Addr::", "Ipv6Addr::"] {
            let wanted = format!("{family}{tier}");
            assert!(
                deciding.contains(&wanted),
                "the policy does not name `{wanted}`, so the tier it belongs to is applied to \
                 one family and not the other: {deciding:?}"
            );
        }
    }
}

/// The web surface can be asked for nothing that names an address.
///
/// The other half of the same property, and the half a reader cannot see: an address
/// built from a constant the policy names is still an address an operator could move
/// if a flag reached it. Held as the whole set rather than as a search for
/// address-sounding words, because the next flag will be called whatever it is
/// called, and a list of words to refuse would have to have guessed it.
///
/// One of them does move where it listens, and what it moves is a **tier** — the two
/// words the policy has addresses for, one of which is refused without a password.
/// So this holds the shape of the answer as well as the set of questions: the field
/// the flag fills is typed as that word and not as an address, which is the
/// difference between choosing between two arrangements somebody argued for and
/// naming one nobody did.
#[test]
fn the_web_surface_can_be_asked_for_nothing_that_names_an_address() {
    let declared: BTreeSet<&str> = ASKED.iter().map(|(flag, _)| *flag).collect();
    let shipped = shipped();
    let Some(parsing) = shipped.get("crates/lemonfiber/src/cli.rs").cloned() else {
        unreachable!("the command line this binary parses lives in the tree this crawl reads")
    };
    assert_eq!(
        fields_of(&parsing, "RawUi"),
        declared,
        "the web surface takes flags this list does not name — say what each settles, and \
         say it having checked that none of them settles the address"
    );
    let Some(serving) = shipped.get("crates/lemonfiber/src/ui.rs").cloned() else {
        unreachable!("the surface this command starts lives in the tree this crawl reads")
    };
    let reach: Vec<&str> = serving
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("pub reach: "))
        .collect();
    assert_eq!(
        reach,
        vec!["Reach,"],
        "what the reach flag fills is not the policy's own word, so something an operator \
         types may be carrying an address"
    );
}

/// Nothing that serves can turn a certificate on.
///
/// A certificate this program made for itself is one a browser warns about, and an
/// operator who learns to click past that warning has been taught something that
/// costs them far more than plain text on a network they trust. So there is none —
/// not switched off, not present — and the transport is said in words instead, which
/// the sentences beside the address are held to.
///
/// This is about the **default**, and a default is only a default while there is
/// something it could be instead. Today there is nothing: the crate that takes the
/// socket carries nothing it could serve TLS with, so a run cannot be talked into
/// one. The day that changes, this is the line that has to be argued away.
#[test]
fn nothing_that_serves_can_turn_on_a_certificate_of_its_own() {
    let read = fs::read_to_string("Cargo.toml").unwrap_or_default();
    // The declarations, not the prose about them: this manifest explains why several
    // of its dependencies are the ones they are, and one of those explanations names
    // a TLS library that is somebody else's dependency.
    let manifest: String = read
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(
        manifest.contains("[dependencies]"),
        "read no manifest for the crate that takes the socket, so this checks nothing"
    );
    let carried: Vec<&&str> = TLS
        .iter()
        .filter(|named| manifest.contains(**named))
        .collect();
    assert!(
        carried.is_empty(),
        "the crate that serves carries what it would take to put a certificate in front \
         of this surface: {carried:?}"
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

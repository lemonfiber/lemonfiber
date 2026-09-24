//! Handing the terminal to the other surface, and what it is given on the way.
//!
//! The one request on this screen that is not work sent to the stack. Everything
//! else here reaches one of the core's commands and is answered; this reaches none,
//! because no other surface has an action for it — a surface cannot start itself, so
//! there is nothing on the web to name and nothing to translate. It is a key rather
//! than an entry on either list for that reason: it belongs beside `q`, which is the
//! other key that ends this screen, and not beside things that are run on the stack.
//!
//! **It ends the screen rather than sharing it.** The web surface announces an
//! address, the warning that the connection is not encrypted, and the token every
//! request to it must carry — eleven lines an operator has to be able to read, copy
//! and come back to. Printed over an alternate screen that is about to be torn down
//! they would be gone; drawn into a box they could not be copied out of a terminal in
//! raw mode. So the screen is given back first and the surface takes an ordinary
//! terminal, where the announcement has room and Ctrl-C means what it says it does.
//!
//! **The question is asked because leaving is what it does.** Nothing on this screen
//! happens on one keypress, and this one is not an exception: what it costs is the
//! dashboard, and an operator who reached for the wrong letter should not lose the
//! screen they were reading.
//!
//! **The five choices are made here, because afterwards there is no screen to make
//! them on.** `lemonfiber ui` takes a port, whether to open a browser, a
//! directory to serve the interface from, whether to set the password this surface
//! asks for, and how far it may be reached, and this key left them all at their
//! defaults — so a screen reached the request and not its arguments, which is the
//! same gap every other partial row on the parity table records. They are made under
//! the question rather than before it: what the surface is about to be given is on
//! the screen the agreement is read on, and `y` goes ahead with exactly what is
//! shown.
//!
//! **They are offered on enter rather than on a key of their own.** This screen
//! answers thirteen letters already, and the five are settings on one request
//! rather than alternatives to choose between — so a list opened before the
//! question would be asking which of them to serve with. Enter is what every list
//! here is taken with and the one key this question had no use for, so the two
//! presses `w` has always taken still take the surface at its defaults.
//!
//! **A port typed here is checked by the only thing that can check one.** The
//! objection this row carried was that a port typed at a screen is a number nothing
//! had checked was free. Nothing can check that and be telling the truth a moment
//! later — a port is free until something takes it — so what the command line does is
//! take it and report what happened, in [`crate::ui::taken`], which is inside the
//! request rather than around it. A port typed here reaches that same bind and is
//! refused by that same problem, on the ordinary terminal this screen has just given
//! back, where the refusal names the address and offers both ways out. What is
//! checked *here* is the other half: that the word is a port at all, which
//! [`crate::ui::Asked::on_port`] answers for every surface that has to turn a word
//! into one.
//!
//! **A browser is worth being able to refuse.** The desktop asked to open one is the
//! host's, not the reader's, and this screen is most useful to somebody at the far
//! end of a remote session — where opening a browser reaches a machine nobody is
//! sitting at, and the line saying one has been opened is false to the person
//! reading it.
//!
//! **A password is answered on the terminal this screen gives back, not on this
//! screen.** The other three are values this box can draw as they are typed, and a
//! password is the one that must not be: every line typed here is drawn by this
//! program into a box, and a box showing the credential in front of the most
//! privileged surface in the product shows it to whoever is standing behind the
//! reader. So the row says only whether one will be asked for, and the asking
//! happens after the handover — the same arrangement the port already has, where
//! what this screen can check is that a word is a port at all and what only the
//! surface can do is take it.
//!
//! **A directory is a path, and this is the one surface a path may be typed at.** A
//! browser is handed a name and never a path, because resolving a caller's path with
//! the server's own authority is a large thing to give away. That argument is about a
//! caller: the operator here is not one. This process was started from their shell,
//! on the host, and it resolves the path with exactly the authority they already had
//! — nothing crosses a boundary, and the same directory is one they could have named
//! with `--assets` a moment earlier. What is served out of it is read-only and is
//! held inside it by [`lemonfiber_core::within`], as it is for `--assets`.

use crate::ui::reach::Reach;
use crate::ui::Asked;

use super::chooser::{Chooser, Listed};
use super::{Press, Stage, Wanted};

/// The key that starts the web surface.
pub(crate) const KEY: char = 'w';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "web";

/// The question, which names what it costs before it costs it.
pub(crate) const ASKS: &str = "Close this screen and start the web interface";

/// What that comes to, in the line under the question.
pub(crate) const ABOUT: &str =
    "it serves to this machine only, and says its address and the word it will ask you for";

/// What the password row says where one is to be asked for.
const ASKED_FOR: &str = "you are asked for one before it starts";

/// What it says where the password is left as it stands.
const LEFT: &str = "left as it is";

/// What the reach row says where the network is asked for.
const NETWORK: &str = "your network, which needs a password set";

/// What it says where it is not.
const MACHINE: &str = "this machine only";

/// What the port row says where no port was named.
const WHICHEVER: &str = "whichever one is free";

/// What the browser row says where one is opened.
const OPENED: &str = "one is opened here when it starts";

/// What it says where none is.
const UNOPENED: &str = "none is opened";

/// What the app row says where the interface built into this program is served.
const BUILT_IN: &str = "the one built into this program";

/// What is asked for above the line a port is typed on.
const PORT: &str = "Which port to listen on, or nothing to be given whichever one is free";

/// What is asked for above the line a directory is typed on.
const DIRECTORY: &str =
    "Which directory to serve the interface from, or nothing for the one built in";

/// Which of the surface's own choices a typed word fills.
///
/// Named rather than assembled, which is the arrangement [`super::question::Narrows`]
/// already has for a read's arguments: what is typed goes into the field
/// [`Asked`] names for it, and what a word has to be to fill that field is that
/// field's own answer — so the refusal an operator reads is the one every surface
/// turning a word into a port reads, rather than one this screen wrote.
pub(super) enum Fills {
    /// The port to listen on.
    Port,
    /// The directory the interface is served from.
    Directory,
}

impl Fills {
    /// What the surface is given, with this choice filled by what was typed.
    fn given(&self, asked: &Asked, said: &str) -> Result<Asked, &'static str> {
        match *self {
            Self::Port => asked.on_port(said),
            Self::Directory => Ok(asked.serving_from(said)),
        }
    }
}

/// Which of the surface's yes-or-no choices a row turns over.
///
/// Named for the same reason [`Fills`] is: what a row does is a fact about that row,
/// and the alternative is one branch here that knows which row it is looking at.
pub(super) enum Turns {
    /// Whether a browser is opened when it starts.
    Browser,
    /// Whether a password is asked for before it starts.
    Password,
    /// How far it may be reached.
    Reach,
}

impl Turns {
    /// What the surface is given, with this choice turned over.
    fn given(&self, asked: &Asked) -> Asked {
        match *self {
            Self::Browser => asked.turned(),
            Self::Password => asked.asking(),
            Self::Reach => asked.reaching(),
        }
    }
}

/// What taking one of the choices does.
///
/// Two shapes, and which one a choice takes is a question about what it holds. A
/// port and a directory are values nothing can list, so they are typed. A browser
/// and a password are one thing or the other — a browser opens or it does not, and a
/// password is either asked for on the way or left alone — and a line to type `yes`
/// on would be a spelling test with two answers.
enum Takes {
    /// A word, under the line it is typed on, filling one of the choices.
    Typed {
        /// What is asked for, above the line it is typed on.
        asks: &'static str,
        /// Which of the choices it fills.
        fills: Fills,
    },
    /// A yes or no, which taking turns over.
    Turned(Turns),
}

/// One of the five choices, and what it is set to.
pub(super) struct Chosen {
    /// What it is called on the row.
    name: &'static str,
    /// What it is set to, in the line beside the name.
    about: String,
    /// What taking the row does.
    takes: Takes,
}

impl Listed for Chosen {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        &self.about
    }
}

/// The five as they stand, the one the list opens on apart from the rest.
///
/// Built from what the surface is about to be given rather than held beside it, so
/// the row an operator reads and the value the surface is started with cannot come
/// to disagree.
pub(super) fn choices(asked: &Asked) -> (Chosen, Vec<Chosen>) {
    (
        Chosen {
            name: "port",
            about: asked
                .port
                .map_or_else(|| WHICHEVER.to_owned(), |port| port.to_string()),
            takes: Takes::Typed {
                asks: PORT,
                fills: Fills::Port,
            },
        },
        vec![
            Chosen {
                name: "browser",
                about: if asked.browser { OPENED } else { UNOPENED }.to_owned(),
                takes: Takes::Turned(Turns::Browser),
            },
            Chosen {
                name: "app",
                about: asked
                    .assets
                    .as_ref()
                    .map_or_else(|| BUILT_IN.to_owned(), |path| path.display().to_string()),
                takes: Takes::Typed {
                    asks: DIRECTORY,
                    fills: Fills::Directory,
                },
            },
            Chosen {
                name: "password",
                about: if asked.password { ASKED_FOR } else { LEFT }.to_owned(),
                takes: Takes::Turned(Turns::Password),
            },
            Chosen {
                name: "reach",
                about: match asked.reach {
                    Reach::Network => NETWORK,
                    Reach::Machine => MACHINE,
                }
                .to_owned(),
                takes: Takes::Turned(Turns::Reach),
            },
        ],
    )
}

/// What is open under the question, where anything is.
///
/// One stage carrying three states rather than three stages, because the three do
/// not run in a line: a choice taken comes back to the question it was read under.
/// Held apart from the screen's own list of stages for the same reason — which of
/// them is open is a fact about this flow.
pub(super) enum Open {
    /// Nothing. The question is the whole of the box.
    Nothing {
        /// Why the last word typed was not taken, where it was not.
        refused: Option<&'static str>,
    },
    /// The five choices, one of them selected.
    Choosing(Chooser<Chosen>),
    /// The line one of their values is typed on.
    Typing {
        /// Which of the five the word being typed fills.
        fills: Fills,
        /// What is asked for, above the line being typed.
        asks: &'static str,
        /// What has been typed of it.
        typed: String,
    },
}

/// The question, over what `lemonfiber ui` is given where no flag says otherwise.
pub(super) fn asking() -> Stage {
    Stage::Handing {
        asked: Asked::unsaid(),
        open: Open::Nothing { refused: None },
    }
}

/// A press, wherever this flow stands.
pub(super) fn handing(stage: &mut Stage, asked: Asked, open: Open, press: &Press) -> Wanted {
    match open {
        Open::Nothing { .. } => agreeing(stage, asked, press),
        Open::Choosing(chooser) => choosing(stage, asked, chooser, press),
        Open::Typing { fills, asks, typed } => typing(stage, asked, fills, asks, typed, press),
    }
}

/// At the question: go ahead, open the five choices, or leave it.
///
/// Only an explicit yes goes ahead, and what it goes ahead with is what the question
/// showed — including a choice that was refused, which the question says so about
/// rather than leaving an operator to find out from the address that was bound.
fn agreeing(stage: &mut Stage, asked: Asked, press: &Press) -> Wanted {
    match *press {
        Press::Typed('y' | 'Y') => Wanted::Serve(asked),
        Press::Accept => {
            let (first, rest) = choices(&asked);
            *stage = Stage::Handing {
                asked,
                open: Open::Choosing(Chooser::over(first, rest)),
            };
            Wanted::Nothing
        }
        _ => Wanted::Nothing,
    }
}

/// Over the five: move, take one, or leave it.
fn choosing(
    stage: &mut Stage,
    asked: Asked,
    mut chooser: Chooser<Chosen>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return take(stage, asked, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Handing {
        asked,
        open: Open::Choosing(chooser),
    };
    Wanted::Nothing
}

/// Turn a choice over, or open the line its value is typed on.
///
/// Either way the question is what comes next, because the question is where what
/// the surface will be given is read.
fn take(stage: &mut Stage, asked: Asked, chosen: Chosen) -> Wanted {
    *stage = match chosen.takes {
        Takes::Turned(turns) => Stage::Handing {
            asked: turns.given(&asked),
            open: Open::Nothing { refused: None },
        },
        Takes::Typed { asks, fills } => Stage::Handing {
            asked,
            open: Open::Typing {
                fills,
                asks,
                typed: String::new(),
            },
        },
    };
    Wanted::Nothing
}

/// Over the line a value is typed on: type, take back, take it, or leave it.
///
/// Nothing typed is a choice rather than an omission: it is what asks for whichever
/// port is free, and for the interface this program was built with. A word that is
/// not one goes back to the question saying why, where what the surface is about to
/// be given is still on the screen to be read.
fn typing(
    stage: &mut Stage,
    asked: Asked,
    fills: Fills,
    asks: &'static str,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            // Read before the question is built rather than in it, so what was typed
            // has become a choice before the one it belongs to is moved into the
            // question it goes back to.
            let filled = fills.given(&asked, &typed);
            *stage = match filled {
                Ok(asked) => Stage::Handing {
                    asked,
                    open: Open::Nothing { refused: None },
                },
                Err(refused) => Stage::Handing {
                    asked,
                    open: Open::Nothing {
                        refused: Some(refused),
                    },
                },
            };
            return Wanted::Nothing;
        }
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => typed.push(character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Handing {
        asked,
        open: Open::Typing { fills, asks, typed },
    };
    Wanted::Nothing
}

#[cfg(test)]
mod tests;

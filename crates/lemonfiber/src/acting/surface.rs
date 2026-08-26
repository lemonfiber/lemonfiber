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
//! **The three choices are made here, because afterwards there is no screen to make
//! them on.** `lemonfiber ui` takes a port, whether to open a browser, and a
//! directory to serve the interface from, and this key left all three at their
//! defaults — so a screen reached the request and not its arguments, which is the
//! same gap every other partial row on the parity table records. They are made under
//! the question rather than before it: what the surface is about to be given is on
//! the screen the agreement is read on, and `y` goes ahead with exactly what is
//! shown.
//!
//! **They are offered on enter rather than on a key of their own.** This screen
//! answers thirteen letters already, and the three are settings on one request
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
//! **A directory is a path, and this is the one surface a path may be typed at.** A
//! browser is handed a name and never a path, because resolving a caller's path with
//! the server's own authority is a large thing to give away. That argument is about a
//! caller: the operator here is not one. This process was started from their shell,
//! on the host, and it resolves the path with exactly the authority they already had
//! — nothing crosses a boundary, and the same directory is one they could have named
//! with `--assets` a moment earlier. What is served out of it is read-only and is
//! held inside it by [`lemonfiber_core::within`], as it is for `--assets`.

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

/// What taking one of the choices does.
///
/// Two shapes, and which one a choice takes is a question about what it holds. A
/// port and a directory are values nothing can list, so they are typed. A browser is
/// one thing or the other, and a line to type `yes` on would be a spelling test with
/// two answers.
enum Takes {
    /// A word, under the line it is typed on, filling one of the choices.
    Typed {
        /// What is asked for, above the line it is typed on.
        asks: &'static str,
        /// Which of the choices it fills.
        fills: Fills,
    },
    /// A yes or no, which taking turns over.
    Turned,
}

/// One of the three choices, and what it is set to.
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

/// The three as they stand, the one the list opens on apart from the rest.
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
                takes: Takes::Turned,
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
    /// The three choices, one of them selected.
    Choosing(Chooser<Chosen>),
    /// The line one of their values is typed on.
    Typing {
        /// Which of the three the word being typed fills.
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

/// At the question: go ahead, open the three choices, or leave it.
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

/// Over the three: move, take one, or leave it.
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
        Takes::Turned => Stage::Handing {
            asked: asked.turned(),
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
mod tests {
    use super::{ABOUT, ASKS, BUILT_IN, KEY, UNOPENED, WHICHEVER};
    use crate::acting::{Acting, Press, Wanted};
    use crate::ui::{Asked, NOT_A_PORT};
    use lemonfiber::reaching::OPENS;

    /// A screen with the question about the web surface open.
    fn asked() -> Acting {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(KEY));
        acting
    }

    /// The same, with the three choices open under it.
    fn changing() -> Acting {
        let mut acting = asked();
        acting.pressed(&Press::Accept);
        acting
    }

    /// Move down the list of choices this many rows.
    fn down(acting: &mut Acting, rows: usize) {
        for _ in 0..rows {
            acting.pressed(&Press::Forward);
        }
    }

    /// Type a word one character at a time and take it.
    fn typing(acting: &mut Acting, word: &str) {
        for letter in word.chars() {
            acting.pressed(&Press::Typed(letter));
        }
        acting.pressed(&Press::Accept);
    }

    /// What the surface would be started with, or nothing where it is not started.
    fn started(acting: &mut Acting) -> Option<Asked> {
        match acting.pressed(&Press::Typed('y')) {
            Wanted::Serve(asked) => Some(asked),
            _ => None,
        }
    }

    /// The box on the screen, as text.
    fn showing(acting: &Acting) -> String {
        acting.pane(20, 100).map_or_else(String::new, |pane| {
            let mut said = vec![pane.title.clone()];
            said.extend(pane.lines.iter().map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<Vec<&str>>()
                    .concat()
            }));
            said.join("\n")
        })
    }

    /// The key is not one the screen already answers, or the thing it already did
    /// stops happening and nothing says so.
    #[test]
    fn the_key_is_not_one_the_screen_already_answers() {
        for taken in [
            'q',
            'r',
            '?',
            'y',
            crate::acting::question::KEY,
            crate::acting::errand::KEY,
            crate::acting::lasting::KEY,
        ] {
            assert_ne!(KEY, taken, "{taken:?} was already spoken for");
        }
        for offer in crate::acting::offer::OFFERED {
            assert_ne!(KEY, offer.key, "{:?} was already spoken for", offer.key);
        }
    }

    /// The question says what it costs, since what it costs is the screen being read.
    #[test]
    fn the_question_says_that_the_screen_goes() {
        assert!(ASKS.contains("Close this screen"));
        assert!(!ABOUT.is_empty());
    }

    /// Nothing happens on one keypress here either, and what a no leaves behind is
    /// the screen that was being read.
    #[test]
    fn only_an_explicit_yes_hands_the_terminal_over() {
        let mut acting = asked();

        assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Typed(KEY)), Wanted::Nothing);
        assert_eq!(
            acting.pressed(&Press::Typed('Y')),
            Wanted::Serve(Asked::unsaid())
        );
    }

    /// The one thing this key is checked against from outside the binary. It reaches
    /// no action and no read, so there is no table of another surface's to hold the
    /// parity row against — the join is the key itself doing what the row claims,
    /// and the published list saying so. Without both, the row would be the only
    /// unheld cell in that column again.
    #[test]
    fn the_request_this_key_reaches_is_published_for_the_parity_table() {
        let mut acting = asked();

        assert_eq!(started(&mut acting), Some(Asked::unsaid()));
        assert_eq!(OPENS, ["ui"]);
    }

    /// The question names the three and what each is set to, and says which key
    /// changes them — a choice nobody is told about is a choice nobody makes.
    #[test]
    fn the_question_names_the_three_choices_and_how_to_change_them() {
        let said = showing(&asked());

        assert!(said.contains(WHICHEVER), "{said}");
        assert!(said.contains(BUILT_IN), "{said}");
        assert!(said.contains("enter changes how it is served"), "{said}");
    }

    /// A port typed at the screen is the port the surface is asked to listen on, and
    /// not merely a keypress the screen accepted.
    #[test]
    fn a_port_typed_at_the_screen_is_the_port_it_is_asked_to_serve_on() {
        let mut acting = changing();
        acting.pressed(&Press::Accept);
        typing(&mut acting, "7171");

        let said = showing(&acting);
        assert!(said.contains("7171"), "{said}");
        assert_eq!(
            started(&mut acting),
            Some(Asked {
                port: Some(7171),
                ..Asked::unsaid()
            })
        );
    }

    /// Naming no port asks for whichever one is free, which is a request rather than
    /// an omission — and it is how a port named by mistake is taken back.
    #[test]
    fn naming_no_port_goes_back_to_whichever_one_is_free() {
        let mut acting = changing();
        acting.pressed(&Press::Accept);
        typing(&mut acting, "7171");
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Accept);
        typing(&mut acting, "");

        assert_eq!(started(&mut acting), Some(Asked::unsaid()));
    }

    /// A word that is not a port is refused in the sentence the request refuses one
    /// with, and nothing is quietly served on a port nobody asked for.
    #[test]
    fn a_word_that_is_not_a_port_is_refused_and_the_question_says_so() {
        let mut acting = changing();
        acting.pressed(&Press::Accept);
        typing(&mut acting, "seventy");

        let said = showing(&acting);
        assert!(said.contains(NOT_A_PORT), "{said}");
        assert!(said.contains(WHICHEVER), "{said}");
        assert_eq!(started(&mut acting), Some(Asked::unsaid()));
    }

    /// The browser is opened unless the screen is told not to, and telling it so is
    /// what changes what the surface is started with.
    #[test]
    fn the_browser_is_opened_unless_the_screen_is_told_otherwise() {
        let mut acting = changing();
        down(&mut acting, 1);
        acting.pressed(&Press::Accept);

        let said = showing(&acting);
        assert!(said.contains(UNOPENED), "{said}");
        assert_eq!(
            started(&mut acting),
            Some(Asked {
                browser: false,
                ..Asked::unsaid()
            })
        );
    }

    /// Turning it back is the same key again, so a row that says one thing is not a
    /// row that can only say it once.
    #[test]
    fn turning_the_browser_back_on_is_the_same_row_again() {
        let mut acting = changing();
        down(&mut acting, 1);
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Accept);
        down(&mut acting, 1);
        acting.pressed(&Press::Accept);

        assert_eq!(started(&mut acting), Some(Asked::unsaid()));
    }

    /// A directory typed at the screen is the directory the interface is served out
    /// of, which is the one choice a browser is deliberately not offered.
    #[test]
    fn a_directory_typed_at_the_screen_is_where_the_interface_is_served_from() {
        let mut acting = changing();
        down(&mut acting, 2);
        acting.pressed(&Press::Accept);
        typing(&mut acting, "/srv/app");

        let said = showing(&acting);
        assert!(said.contains("/srv/app"), "{said}");
        assert_eq!(
            started(&mut acting),
            Some(Asked {
                assets: Some(std::path::PathBuf::from("/srv/app")),
                ..Asked::unsaid()
            })
        );
    }

    /// Moving back up the list is moving, and the row the cursor lands on is the row
    /// enter takes — the same movement every other list on this screen has.
    #[test]
    fn the_cursor_moves_over_the_three_and_stays_where_the_list_begins() {
        let mut acting = changing();
        down(&mut acting, 2);
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Back);
        // Past the top is still the top, so this takes the first row.
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Accept);
        typing(&mut acting, "7171");

        assert_eq!(
            started(&mut acting),
            Some(Asked {
                port: Some(7171),
                ..Asked::unsaid()
            })
        );
    }

    /// A key the list has no use for leaves the list where it was, rather than
    /// closing it under somebody who mistyped.
    #[test]
    fn a_key_the_list_has_no_use_for_leaves_it_open() {
        let mut acting = changing();
        acting.pressed(&Press::Typed('z'));
        acting.pressed(&Press::Rubout);
        acting.pressed(&Press::Accept);
        typing(&mut acting, "7171");

        assert_eq!(
            started(&mut acting),
            Some(Asked {
                port: Some(7171),
                ..Asked::unsaid()
            })
        );
    }

    /// A character taken back is a character not typed, and moving where there is
    /// nothing to move over leaves the line as it was.
    #[test]
    fn a_character_taken_back_is_not_part_of_the_port() {
        let mut acting = changing();
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Forward);
        for letter in "71719".chars() {
            acting.pressed(&Press::Typed(letter));
        }
        acting.pressed(&Press::Rubout);
        acting.pressed(&Press::Accept);

        assert_eq!(
            started(&mut acting),
            Some(Asked {
                port: Some(7171),
                ..Asked::unsaid()
            })
        );
    }

    /// Backing out of the choices, or out of the line under them, leaves the screen
    /// that was being read and starts nothing.
    #[test]
    fn backing_out_of_the_choices_starts_nothing() {
        let mut leaving_the_list = changing();
        assert_eq!(leaving_the_list.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(
            showing(&leaving_the_list).is_empty(),
            "the box went with it"
        );
        assert_eq!(started(&mut leaving_the_list), None);

        let mut leaving_the_line = changing();
        leaving_the_line.pressed(&Press::Accept);
        assert_eq!(leaving_the_line.pressed(&Press::Abandon), Wanted::Nothing);
        assert_eq!(started(&mut leaving_the_line), None);
    }

    /// The line a value is typed on says what is being asked for, since a line with
    /// nothing above it is a line nobody knows what to put on.
    #[test]
    fn the_line_a_value_is_typed_on_says_what_it_is_for() {
        let mut acting = changing();
        acting.pressed(&Press::Accept);
        let port = showing(&acting);
        assert!(port.contains("Which port"), "{port}");

        let mut acting = changing();
        down(&mut acting, 2);
        acting.pressed(&Press::Accept);
        let directory = showing(&acting);
        assert!(directory.contains("Which directory"), "{directory}");
    }
}

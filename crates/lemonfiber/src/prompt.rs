//! The terminal that asks the operator setup's questions.
//!
//! This is the reading, rendering half of the wizard's [`Prompt`] port: the core
//! decides what to ask and what each answer means, and this turns a question into
//! a line on the terminal and a typed line back. It offers only the choices that
//! apply where it runs, so an answer the wizard would reject is never gathered.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use lemonfiber_core::app::setup::{Prompt, StorageWarning};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::prerequisites::PrerequisiteMap;
use lemonfiber_core::storage::COPY_CONSEQUENCE;
use lemonfiber_core::wizard::{Library, Plan};

/// A prompt that reads the operator's answers from the terminal.
pub struct Terminal {
    environment: Environment,
    default_data: PathBuf,
}

impl Terminal {
    /// A terminal prompt for `environment`, proposing `default_data` where the
    /// operator does not name a data location of their own.
    pub const fn new(environment: Environment, default_data: PathBuf) -> Self {
        Self {
            environment,
            default_data,
        }
    }
}

impl Prompt for Terminal {
    fn protocols(&self) -> Protocols {
        println!("\nHow will you fetch content?");
        println!("  1) Usenet only");
        println!("  2) Torrents only");
        println!("  3) Both");
        println!("  4) Neither — serve an existing library only");
        match ask("Choose [3]:").as_str() {
            "1" => Protocols {
                usenet: true,
                torrent: false,
            },
            "2" => Protocols {
                usenet: false,
                torrent: true,
            },
            "4" => Protocols {
                usenet: false,
                torrent: false,
            },
            _ => Protocols::both(),
        }
    }

    fn prerequisites(&self, map: &PrerequisiteMap) {
        // Nothing required is stated first and plainly — a folder of existing media
        // reaching a working Jellyfin with no accounts is an end state, not a lesser
        // one. Otherwise each thing is named, explained, costed in a band, and given
        // the criteria that decide it — no vendors, since those age and vary.
        if let Some(note) = map.library_only {
            println!("\n{note}");
            return;
        }

        println!("\nBefore the questions that follow, here is what your choices will need.");
        println!("You can go and get these, then run setup again — it remembers your answers.\n");
        for item in &map.items {
            println!("  {}", item.label);
            println!("    What it is: {}", item.what);
            println!("    Why:        {}", item.why);
            println!("    Cost:       {}", item.cost.phrase());
            println!("    Look for:");
            for criterion in &item.criteria {
                println!("      · {criterion}");
            }
            println!("    Without it: {}\n", item.without);
        }
        let _ = ask("Press enter when you have noted these.");
    }

    fn data_location(&self) -> PathBuf {
        let shown = self.default_data.display();
        let answer = ask(&format!(
            "\nWhere should the library and downloads live? [{shown}]:"
        ));
        if answer.is_empty() {
            self.default_data.clone()
        } else {
            PathBuf::from(answer)
        }
    }

    fn hardlinks(&self, path: &Path, inferred_from: Option<&Path>) {
        match inferred_from {
            // Tested directly: the chosen location itself proved it links.
            None => println!(
                "  ✓ {} hardlinks — imports will be instant and cost no extra disk.",
                path.display()
            ),
            // The location does not exist yet, so its parent's filesystem stood in
            // for it. Say so, rather than present a parent's answer as proven of a
            // path never touched — a separate drive mounted here later could differ,
            // and the storage check re-tests the real location once it exists.
            Some(parent) => println!(
                "  ✓ {} will hardlink — its filesystem ({}) does. If it becomes a \
                 separate drive, that is checked when the stack first runs.",
                path.display(),
                parent.display()
            ),
        }
    }

    fn storage_warning(&self, path: &Path, warning: &StorageWarning) -> bool {
        match warning {
            StorageWarning::CopyOnly { limitation } => {
                match limitation {
                    Some(reason) => println!("  ✗ {} cannot hardlink — {reason}.", path.display()),
                    None => println!("  ✗ {} cannot hardlink.", path.display()),
                }
                // The consequence is stated in the same words a later diagnosis
                // would use, indented so it reads as the explanation of the line
                // above rather than a new claim.
                println!("    {COPY_CONSEQUENCE}");
            }
            StorageWarning::Untested { reason } => {
                println!(
                    "  ? {} could not be tested for hardlinks — {reason}.",
                    path.display()
                );
            }
        }
        // Defaulting to no nudges the operator toward a location that links,
        // without taking the choice away — some know their setup and mean it.
        yes_no("\nUse this location anyway?", false)
    }

    fn service_user(&self) -> Option<(u32, u32)> {
        println!("\nThe containers can run as a chosen user, so the files they create are yours.");
        parse_ids(&ask(
            "User and group as UID:GID, or blank to keep the image default:",
        ))
    }

    fn library(&self) -> Library {
        let native = self.environment.offers_native_jellyfin();
        println!("\nServe your library with Jellyfin?");
        println!("  1) Yes, in a container — works everywhere");
        if native {
            println!("  2) Yes, on the host — reaches a hardware transcoder the container cannot");
        }
        println!("  3) No media server");
        match ask("Choose [1]:").as_str() {
            "2" if native => Library::JellyfinNative,
            "3" => Library::None,
            _ => Library::JellyfinDocker,
        }
    }

    fn household(&self) -> bool {
        yes_no("\nWill others in your home use it?", false)
    }

    fn autostart(&self) -> bool {
        yes_no("\nStart the stack when this machine boots?", false)
    }

    fn confirm(&self, plan: &Plan) -> bool {
        println!("\nThis is what setup will write:");
        for (key, value) in plan.settings() {
            println!("  {key} = {value}");
        }
        yes_no("\nApply it?", true)
    }
}

/// Print a question and read the operator's trimmed answer, empty on end-of-input.
fn ask(question: &str) -> String {
    print!("{question} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_owned()
}

/// Ask a yes-or-no question, taking the default where the answer is neither.
fn yes_no(question: &str, default: bool) -> bool {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    match ask(&format!("{question} {hint}:")).to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}

/// Read a `UID:GID` pair, or nothing where it is blank or malformed.
fn parse_ids(answer: &str) -> Option<(u32, u32)> {
    let (uid, gid) = answer.split_once(':')?;
    Some((uid.trim().parse().ok()?, gid.trim().parse().ok()?))
}

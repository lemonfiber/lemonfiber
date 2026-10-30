//! Which one service the household is sent to, and the honest answer where there
//! is none.
//!
//! The stack publishes several things to the local network, and left to improvise an
//! operator sends three links and a paragraph explaining which is which — the exact
//! complexity this product exists to remove. So the question has one answer, derived
//! here rather than decided again by each surface that shows it.
//!
//! The two that can be a door are told apart by the contract the adapter in front of
//! them speaks, never by their name or by an API this build holds an adapter for: a
//! service fronted by one speaking `request.intake` is where asking begins, and one
//! fronted by one speaking `media.serve` is a library, or a shelf where the adapter
//! declares [`SHELF_MEDIA`] alone, so any service speaking the same contract resolves the
//! same way. The stack's guard in front of the media server is
//! met as the library it guards, under that library's name. The rest speak no such
//! contract, so each of the stack's is written down below with what it is to the
//! household — and one nobody has written down, another plugin's among them, is not
//! offered, for the reason [`crate::config::display`] withholds a setting nobody vouched
//! for.
//!
//! All of which is a default rather than a decree. An operator who disagrees about
//! their own stack names the service they want, and [`chosen`] is where that name
//! meets the one bound on it: a setting may pick between the places this stack
//! already publishes to the household, and may not turn into a way past the tiers.

mod address;
mod chosen;
pub(crate) mod run;

use serde::Serialize;

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::proxy::front;
use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_contract::capabilities::stream::guard;
use lemonfiber_manifest::{Bind, Service};

use crate::plugin::{Installed, Placed};

pub use address::{address, proxied, publishes_a_name, Address, Place, ONLY_HERE};
pub use chosen::{chosen, Chosen, Refusal, KEPT};

/// What a service published to the local network is to the people in the house.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Facing {
    /// Where asking for something begins, and where what was asked for is followed.
    /// It links onward to the library, which is what makes it a place to start
    /// rather than one more address to be sent.
    Asking,
    /// The library, where what arrived is watched. A place to start only where there
    /// is nothing to ask for, because somebody sent there has no way to ask.
    Watching,
    /// One kind of media and no other, reached from the library rather than instead
    /// of it.
    Shelf,
    /// An index over every service, including the ones nobody in the house should
    /// learn exist.
    Operators,
    /// How the others are reached, rather than one of them.
    Carriage,
    /// Published to the household, and nothing here says what it is to them.
    Unstated,
}

impl Facing {
    /// Why this is, or is not, somewhere for the household to begin.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Self::Asking => {
                "where a request begins, and where what was asked for is followed — and \
                 it links onward to the library"
            }
            Self::Watching => {
                "where the library is watched, and no way to ask for anything that is \
                 not in it yet"
            }
            Self::Shelf => "one kind of media, reached from the library rather than instead of it",
            Self::Operators => {
                "an index over every service, including the ones nobody in the house \
                 should learn exist — the operator's convenience, never a way in"
            }
            Self::Carriage => "how the others are reached, rather than one of them",
            Self::Unstated => {
                "nothing here says what this is to the household, so it is not offered \
                 as somewhere to begin"
            }
        }
    }

    /// Whether the household can be sent here as a place to start.
    #[must_use]
    pub const fn begins(self) -> bool {
        matches!(self, Self::Asking | Self::Watching)
    }

    /// Whether somebody in the house may be handed the way there at all.
    ///
    /// Wider than [`Self::begins`] and narrower than every service there is. A shelf
    /// is not somewhere to *start* — it holds one kind of media and is reached from
    /// the library — but a member sent to it arrives somewhere meant for them, so an
    /// address is a thing they can be given.
    ///
    /// **The other three are named in order to be refused by name.** The index over
    /// every service exists for the operator and includes the ones nobody in the
    /// house should learn exist; the carriage is how the others are reached rather
    /// than one of them; and a service nothing has said anything about is offered to
    /// nobody, which is the register's own default and the safe direction to be
    /// wrong in. Naming one of those and handing over the way to it are different
    /// acts, and only the first is what standing beside the door means.
    #[must_use]
    pub const fn handed_over(self) -> bool {
        matches!(self, Self::Asking | Self::Watching | Self::Shelf)
    }
}

/// The media in [`lemonfiber_manifest::container::MEDIA`] that make a media server a
/// [`Facing::Shelf`] where it declares nothing else: one kind of media, reached from the
/// library rather than instead of it.
pub const SHELF_MEDIA: [&str; 5] = ["music", "books", "comics", "audiobooks", "podcasts"];

/// The services published to the household that a contract does not settle.
///
/// A register rather than a rule read off a name, for the reason
/// [`crate::config::display::SHOWN`] is one: a guess is wrong in whichever direction
/// nobody chose, and the service that ends up wrongly offered to a household is by
/// definition the one nobody thought about. So an entry is a decision somebody made
/// and can be reviewed on; anything else is [`Facing::Unstated`] and is offered to
/// nobody. It names no place to begin: those two are the contracts'.
const NAMED: &[(&str, Facing)] = &[("homepage", Facing::Operators)];

/// What one of the stack's own services is to the household, or nothing where it is not
/// published to them at all.
///
/// Publication is the manifest's own [`Bind`], so a service reachable only from this
/// machine is not a candidate for anything here — which is the same fact the admin
/// tier's loopback binding rests on, read from the same field rather than from a
/// second list that could disagree with it. A service guarding the media stream is how
/// the household reaches the library, and the household's proxy is how the others are
/// reached; anything else is what the register says.
#[must_use]
pub fn facing(service: &Service) -> Option<Facing> {
    if service.bind != Some(Bind::Lan) {
        return None;
    }
    let provides = |capability: &str| service.provides.iter().any(|one| one == capability);
    if provides(guard::CAPABILITY) {
        return Some(Facing::Watching);
    }
    if provides(front::CAPABILITY) {
        return Some(Facing::Carriage);
    }
    Some(named(&service.id))
}

/// What a plugin's service is to the household, or nothing where it is not published to
/// them at all.
///
/// A first-party part moved in place of the stack's guard of the media stream is how the
/// household reaches the library, as the stack's own is; a plugin declaring that it
/// guards the stream is not thereby the way in. Anything else is judged by the contract
/// the adapter of `installed` in front of it speaks. Where none settles it, a first-party
/// part moved in place of one of the stack's services is what the register says of the
/// service it stands in for, and any other plugin's service is unstated: a plugin's
/// service choosing an id the register names is not thereby what the register says.
#[must_use]
pub fn brought(installed: &[Installed], plugin: &Installed, placed: &Placed) -> Option<Facing> {
    placed.reached.as_ref()?.hostname()?;
    if guards_in_place(installed, placed) {
        return Some(Facing::Watching);
    }
    Some(by_contract(plugin, placed).unwrap_or_else(|| {
        if crate::project::moved_in_place(installed, &placed.service) {
            named(&placed.service)
        } else {
            Facing::Unstated
        }
    }))
}

/// Whether `placed` guards the media stream as a first-party part moved in place of one of
/// the stack's services, which is the only way a plugin's service is trusted to: facing
/// the household is decided by who a part is, not by what it declares.
fn guards_in_place(installed: &[Installed], placed: &Placed) -> bool {
    crate::project::moved_in_place(installed, &placed.service)
        && placed.provides.iter().any(|one| one == guard::CAPABILITY)
}

/// What the contract the adapter of `plugin` in front of `placed` speaks makes it to the
/// household, where it settles it.
fn by_contract(plugin: &Installed, placed: &Placed) -> Option<Facing> {
    let speaking = |capability: &str, major: u32| {
        let spoken = lemonfiber_contract::spoken(capability, major);
        plugin.services.iter().find(|adapter| {
            adapter.fronts.as_deref() == Some(placed.service.as_str())
                && adapter.speaks.contains(&spoken)
        })
    };
    if speaking(intake::CAPABILITY, intake::MAJOR).is_some() {
        return Some(Facing::Asking);
    }
    if speaking(front::CAPABILITY, front::MAJOR).is_some() {
        return Some(Facing::Carriage);
    }
    speaking(serve::CAPABILITY, serve::MAJOR).map(|adapter| served(&adapter.media_types))
}

/// What a media server declaring `media` is to the household: a shelf where it declares
/// shelf media alone, and the library otherwise.
fn served(media: &[String]) -> Facing {
    if !media.is_empty() && media.iter().all(|one| SHELF_MEDIA.contains(&one.as_str())) {
        Facing::Shelf
    } else {
        Facing::Watching
    }
}

/// How the household reaches one service published to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reached<'a> {
    /// At this machine, on the port the stack publishes it on, where it publishes one.
    Port(Option<u16>),
    /// Through the stack's proxy, at this one label in front of the operator's domain.
    Proxied(&'a str),
}

/// One service the household might be sent to, the stack's or an installed plugin's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate<'a> {
    /// The id it runs under, which a front door is named by.
    pub id: &'a str,
    /// What it is called in front of a person.
    pub name: &'a str,
    /// What it is to the household, or nothing where it is not published to them.
    pub facing: Option<Facing>,
    /// How the household reaches it.
    pub reached: Reached<'a>,
}

/// Every service the household might be sent to: the stack's, in the order it declares
/// them, then each installed plugin's. A service of the stack a first-party part has
/// moved in place of is met as the part's, once.
///
/// The stack's guard in front of the media server is met under the name of the library
/// it guards: the upstream behind whatever its ask for `media.serve` settles on among
/// `fillers`, which is what the household arrives at through it.
#[must_use]
pub fn candidates<'a>(
    services: &'a [Service],
    installed: &'a [Installed],
    fillers: &crate::wiring::Fillers,
) -> Vec<Candidate<'a>> {
    let bundled = services
        .iter()
        .filter(|service| !crate::project::moved_in_place(installed, &service.id))
        .map(move |service| Candidate {
            id: &service.id,
            name: guarded(
                services,
                installed,
                fillers,
                (&service.id, &service.provides),
            )
            .unwrap_or(&service.name),
            facing: facing(service),
            reached: Reached::Port(service.port),
        });
    let plugins = installed.iter().flat_map(move |plugin| {
        plugin.services.iter().map(move |placed| Candidate {
            id: &placed.service,
            name: guards_in_place(installed, placed)
                .then(|| {
                    guarded(
                        services,
                        installed,
                        fillers,
                        (&placed.service, &placed.provides),
                    )
                })
                .flatten()
                .unwrap_or_else(|| placed.called()),
            facing: brought(installed, plugin, placed),
            reached: match placed.reached.as_ref() {
                Some(reached) if crate::project::moved_in_place(installed, &placed.service) => {
                    Reached::Port(Some(reached.port()))
                }
                Some(reached) => reached
                    .hostname()
                    .map_or(Reached::Port(None), Reached::Proxied),
                None => Reached::Port(None),
            },
        })
    });
    bundled.chain(plugins).collect()
}

/// The name of the library the service `guard`, by its id and what it provides, guards,
/// where it guards the media stream and its ask for `media.serve` settles on one service
/// among `fillers`: the upstream that service fronts, or the service itself.
fn guarded<'a>(
    services: &'a [Service],
    installed: &'a [Installed],
    fillers: &crate::wiring::Fillers,
    (id, provides): (&str, &[String]),
) -> Option<&'a str> {
    if !provides.iter().any(|one| one == guard::CAPABILITY) {
        return None;
    }
    let [filler] = fillers
        .asks()
        .iter()
        .find(|ask| ask.by == id && ask.capability == serve::CAPABILITY)?
        .fillers
        .as_slice()
    else {
        return None;
    };
    let library = fillers.fronted_by(filler).unwrap_or(filler).id.as_str();
    installed
        .iter()
        .flat_map(|plugin| &plugin.services)
        .find(|placed| placed.service == library)
        .map(Placed::called)
        .or_else(|| {
            services
                .iter()
                .find(|service| service.id == library)
                .map(|service| service.name.as_str())
        })
}

/// What the register says this service is, or that nobody has said.
fn named(id: &str) -> Facing {
    NAMED
        .iter()
        .find(|(named, _)| *named == id)
        .map_or(Facing::Unstated, |(_, facing)| *facing)
}

/// The one service the household begins at, from everything the stack declares and
/// every installed plugin brings.
///
/// Whichever request surface there is, and the library where there is none; the
/// stack's own first where both it and a plugin offer one. Read from what is
/// *declared* rather than from what is up at this moment,
/// deliberately: a request service that is not running is a front door that is down,
/// and answering "the library, then" would hand the household somewhere they cannot
/// ask for anything without ever saying that is what happened.
///
/// Nothing where the stack publishes neither. That is an answer — an operator-only
/// configuration has no household front door — and it is said as one rather than
/// filled in with the nearest thing that would open.
#[must_use]
pub(crate) fn begins_at<'a, 'b>(
    candidates: &'b [Candidate<'a>],
) -> Option<(Facing, &'b Candidate<'a>)> {
    let mut best: Option<(Facing, &Candidate<'a>)> = None;
    for candidate in candidates {
        let Some(facing) = candidate.facing.filter(|facing| facing.begins()) else {
            continue;
        };
        if facing == Facing::Asking {
            return Some((facing, candidate));
        }
        if best.is_none() {
            best = Some((facing, candidate));
        }
    }
    best
}

/// A manifest service built for a test, for the two modules that need one.
///
/// Beside the rule rather than inside either test module: what a stack declares is
/// this file's vocabulary, and a second copy of a literal that names every field
/// the schema declares is a second thing to keep agreeing with it.
#[cfg(test)]
pub(crate) mod fixtures;

#[cfg(test)]
mod tests;

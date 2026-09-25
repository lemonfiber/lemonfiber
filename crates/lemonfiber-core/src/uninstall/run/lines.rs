//! Turning what was read into the list an operator checks.
//!
//! One line per thing, each said to be going or said to be kept and why. Never a
//! count: "19 containers and 42 GiB" is something to agree to, and a list is
//! something to check — and the checking is the whole point of showing it first.
//!
//! A line that is kept is still a line. An image another project stands on, a path
//! this run could not confirm the location of, a container an unreachable engine is
//! still holding: leaving those off would tell an operator the machine is clean when
//! it is not, which is the failure the manifest exists to prevent.

use crate::uninstall::{ours, Foreign, Item, Sort, Tier};

use super::gathering::{network_of, Gathered};

/// The reason an image is left where it is.
const SHARED: &str = "another project on this machine is standing on it";

/// The reason anything the engine holds is left where it is.
const NO_ENGINE: &str = "the container engine could not be reached";

/// The reason lemonfiber's own files are left where they are.
const NOWHERE_KNOWN: &str =
    "this run could not confirm where lemonfiber keeps its own files, and a guess at \
     the usual place could name somebody else's directory";

/// The reason a data location is left as one tree.
const NOT_ALL_OURS: &str =
    "there are files beneath it the stack did not put there, so it is never removed as \
     one tree";

/// Every line this tier reaches, in the order they are worth reading.
pub(super) fn items(
    tier: Tier,
    project: &str,
    gathered: &Gathered,
    foreign: &[Foreign],
) -> Vec<Item> {
    match tier {
        Tier::Stop => stopping(gathered),
        Tier::Services => services(project, gathered),
        Tier::Configuration => configuration(gathered),
        Tier::Media => media(gathered, foreign),
    }
}

/// A container, going or kept.
fn container(name: &str, kept: Option<&str>) -> Item {
    Item {
        name: name.to_owned(),
        sort: Sort::Container,
        what: "a container this stack is running".to_owned(),
        bytes: None,
        kept: kept.map(str::to_owned),
        secret: false,
    }
}

/// The containers, by the name the engine gives them where it answered and by the
/// name Compose would give them where it did not.
fn containers(project: &str, gathered: &Gathered) -> Vec<String> {
    if gathered.engine_answered {
        return gathered.containers.clone();
    }
    gathered
        .services
        .iter()
        .map(|service| format!("{project}-{}", service.id))
        .collect()
}

/// Stopping removes nothing, so every line it reaches is a line it keeps.
///
/// Listed all the same. An operator choosing the tier that removes nothing is owed
/// the same list as one choosing any other, or "nothing is removed" is a claim they
/// have to take on trust.
fn stopping(gathered: &Gathered) -> Vec<Item> {
    gathered
        .containers
        .iter()
        .map(|name| container(name, Some("stopping leaves it where it is")))
        .collect()
}

/// The containers, the network they were on, and the images that were pulled.
fn services(project: &str, gathered: &Gathered) -> Vec<Item> {
    let unreachable = (!gathered.engine_answered).then_some(NO_ENGINE);
    let mut lines: Vec<Item> = containers(project, gathered)
        .iter()
        .map(|name| container(name, unreachable))
        .collect();

    lines.push(Item {
        name: network_of(project),
        sort: Sort::Network,
        what: "the network the containers were on".to_owned(),
        bytes: None,
        kept: unreachable.map(str::to_owned),
        secret: false,
    });
    lines.extend(images(gathered, project));
    lines
}

/// One line per image this stack declares, with what it occupies and who else is
/// standing on it.
///
/// An image nothing here declares is not this stack's to remove and is not listed at
/// all: a machine's image cache is the engine's, and an uninstall that went through
/// it would be removing other people's things.
fn images(gathered: &Gathered, project: &str) -> Vec<Item> {
    gathered
        .services
        .iter()
        .map(|service| format!("{}:{}", service.image, service.tag))
        .collect::<std::collections::BTreeSet<String>>()
        .into_iter()
        .filter_map(|reference| image(gathered, &reference, project))
        .collect()
}

/// One declared image as a line, or nothing where the engine answered and does not
/// have it.
fn image(gathered: &Gathered, reference: &str, project: &str) -> Option<Item> {
    let pulled = gathered
        .images
        .iter()
        .find(|image| image.tags.iter().any(|tag| tag == reference));

    let (bytes, kept) = match pulled {
        Some(image) => (
            Some(image.bytes),
            shared(&image.projects, project).then_some(SHARED),
        ),
        // The engine answered and does not have it, so there is nothing to remove.
        None if gathered.images_answered => return None,
        None => (None, Some(NO_ENGINE)),
    };

    Some(Item {
        name: reference.to_owned(),
        sort: Sort::Image,
        what: "an image pulled for one of this stack's services".to_owned(),
        bytes,
        kept: kept.map(str::to_owned),
        secret: false,
    })
}

/// Whether anything but this project is standing on an image.
///
/// A container under no Compose project counts, and is the case worth being careful
/// about: something the operator started by hand is exactly as broken by a removal
/// as another project's stack, and has nobody to notice.
fn shared(projects: &[String], ours: &str) -> bool {
    projects.iter().any(|project| project != ours)
}

/// lemonfiber's own two directories, and each thing kept beneath them.
///
/// The directories carry the size, because they are what is removed. Each thing
/// beneath them is a line of its own with no size, so the list says what is being
/// destroyed — the credentials most of all — without counting the same bytes twice.
fn configuration(gathered: &Gathered) -> Vec<Item> {
    let Some(paths) = gathered.paths.as_ref() else {
        // Named at the locations this kind of machine conventionally uses, so an
        // operator is told where to look — and every one of them kept, because a
        // path this run had to fall back to is one it must not remove.
        let usual = conventional();
        return crate::stored::EVERY
            .iter()
            .map(|entry| Item {
                name: (entry.at)(&usual).display().to_string(),
                sort: Sort::Path,
                what: entry.what.to_owned(),
                bytes: None,
                kept: Some(NOWHERE_KNOWN.to_owned()),
                secret: entry.secret,
            })
            .collect();
    };

    let mut lines: Vec<Item> = [
        (
            paths.config_dir(),
            "your answers, and what lemonfiber wrote",
        ),
        (paths.data_dir(), "what lemonfiber can make again"),
    ]
    .into_iter()
    .map(|(root, what)| Item {
        name: root.display().to_string(),
        sort: Sort::Path,
        what: what.to_owned(),
        bytes: Some(beneath(gathered, root)),
        kept: None,
        // The directory is not itself a credential. What holds one is named below,
        // each on its own line, so what a run reports as destroyed is a list of
        // credentials rather than a list of directories.
        secret: false,
    })
    .collect();

    lines.extend(crate::stored::EVERY.iter().map(|entry| Item {
        name: (entry.at)(paths).display().to_string(),
        sort: Sort::Path,
        what: entry.what.to_owned(),
        bytes: None,
        kept: None,
        secret: entry.secret,
    }));
    lines
}

/// The layout this kind of machine conventionally uses, for a run that could not
/// resolve its own.
///
/// Written with a `~` rather than an expanded home directory, because expanding one
/// means asking the operating system and this is the code path taken precisely when
/// that did not work.
fn conventional() -> crate::config::paths::Paths {
    crate::config::paths::Paths::rooted(
        std::path::Path::new("~/.config"),
        std::path::Path::new("~/.local/share"),
    )
}

/// What the walk found beneath a directory.
fn beneath(gathered: &Gathered, root: &std::path::Path) -> u64 {
    gathered
        .kept
        .iter()
        .filter(|occupant| occupant.path.starts_with(root))
        .map(|occupant| occupant.bytes)
        .fold(0, u64::saturating_add)
}

/// The library and the downloads.
///
/// One line for the data location where everything beneath it is the stack's, and
/// one line per directory the stack owns where anything else is there. That is what
/// "files the stack did not create prevent blanket deletion" means as a shape rather
/// than as a warning: the tree is not on the list at all once something of the
/// operator's is in it.
fn media(gathered: &Gathered, found: &[Foreign]) -> Vec<Item> {
    let Some(root) = gathered.root.as_ref() else {
        return Vec::new();
    };

    let types: Vec<String> = gathered
        .services
        .iter()
        .flat_map(|service| service.media_types.clone())
        .collect();

    let mut lines = vec![Item {
        name: root.display().to_string(),
        sort: Sort::Path,
        what: "the data location, with everything beneath it".to_owned(),
        bytes: Some(beneath_data(gathered)),
        kept: (!found.is_empty()).then(|| NOT_ALL_OURS.to_owned()),
        secret: false,
    }];

    if found.is_empty() {
        return lines;
    }

    lines.extend(ours(&types).into_iter().filter_map(|directory| {
        let at = root.join(&directory);
        let bytes = gathered
            .walked
            .iter()
            .filter(|occupant| occupant.path.starts_with(&at))
            .map(|occupant| occupant.bytes)
            .fold(0, u64::saturating_add);
        (bytes > 0).then(|| Item {
            name: at.display().to_string(),
            sort: Sort::Path,
            what: format!("the stack's own {directory}"),
            bytes: Some(bytes),
            kept: None,
            secret: false,
        })
    }));
    lines
}

/// What the walk found beneath the data location.
fn beneath_data(gathered: &Gathered) -> u64 {
    gathered
        .walked
        .iter()
        .map(|occupant| occupant.bytes)
        .fold(0, u64::saturating_add)
}

//! A tree a test described rather than one it had to create.
//!
//! What the reckoning above this port turns on is not any real disk's contents but
//! the shape of them: a file with two names, one with one, an archive beside what
//! was unpacked from it. Writing those out on a real filesystem means creating
//! hardlinks in a temporary directory, which works on the machines it works on and
//! is a different test on the ones it does not.
//!
//! So the tree is a list of entries the test wrote down, answered for whichever
//! root is asked about — a walk is a walk of what is beneath a path, so an entry
//! that is not beneath the one asked for is not part of the answer.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::filesystem::Fault;
use lemonfiber_ports::occupancy::{Occupancy, Occupant};

/// A walk over a tree a test wrote down.
pub struct Walking {
    held: Vec<Occupant>,
    refuses: Option<String>,
}

impl Walking {
    /// A tree holding exactly these files.
    #[must_use]
    pub fn holding(held: Vec<Occupant>) -> Arc<Self> {
        Arc::new(Self {
            held,
            refuses: None,
        })
    }

    /// A tree that is there and will not be read, in the platform's words.
    #[must_use]
    pub fn refusing(why: &str) -> Arc<Self> {
        Arc::new(Self {
            held: Vec::new(),
            refuses: Some(why.to_owned()),
        })
    }
}

#[async_trait]
impl Occupancy for Walking {
    async fn beneath(&self, root: &Path) -> Result<Vec<Occupant>, Fault> {
        if let Some(why) = &self.refuses {
            return Err(Fault::new(why.clone()));
        }
        Ok(self
            .held
            .iter()
            .filter(|occupant| occupant.path.starts_with(root))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use lemonfiber_ports::filesystem::Identity;
    use lemonfiber_ports::occupancy::{Occupancy, Occupant};

    use super::Walking;

    /// A file at a path, whose size and identity no case here turns on.
    fn file(path: &str) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes: 1,
            identity: Some(Identity { file: 1, links: 1 }),
        }
    }

    #[tokio::test]
    async fn a_walk_answers_with_what_is_beneath_the_path_asked_about() {
        let walking = Walking::holding(vec![file("/srv/media/a.mkv"), file("/elsewhere/b.mkv")]);
        let found = walking.beneath(Path::new("/srv/media")).await;
        assert_eq!(found, Ok(vec![file("/srv/media/a.mkv")]));
        assert_eq!(walking.beneath(Path::new("/nowhere")).await, Ok(Vec::new()));
    }

    #[tokio::test]
    async fn a_tree_that_will_not_be_read_says_so_in_the_platforms_words() {
        let refused = Walking::refusing("permission denied")
            .beneath(Path::new("/srv/media"))
            .await;
        assert!(refused.is_err_and(|fault| fault.message == "permission denied"));
    }
}

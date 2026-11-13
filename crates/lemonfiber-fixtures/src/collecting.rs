//! A curator of music and of books that writes down what it is asked.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::service::{
    AskedWith, BookAsk, BookCurating, BookUnit, Candidate, CandidateState, Candidates, Failure,
    MusicAsk, MusicCurating, MusicUnit, Profile,
};

/// A curator answering every ask with one candidate, holding the folders and profiles it
/// was built with, and recording each question it was asked.
#[derive(Default)]
pub struct Curator {
    with: AskedWith,
    asked: Mutex<Vec<String>>,
}

impl Curator {
    /// A curator holding one root folder and one quality profile, and that profile as its
    /// metadata profile where `metadata`.
    #[must_use]
    pub fn holding(metadata: bool) -> Arc<Self> {
        let profile = Profile {
            id: 3,
            name: "Standard".to_owned(),
        };
        Arc::new(Self {
            with: AskedWith {
                units: Vec::new(),
                root_folders: vec!["/library".to_owned()],
                quality: vec![profile.clone()],
                metadata: if metadata { vec![profile] } else { Vec::new() },
            },
            asked: Mutex::new(Vec::new()),
        })
    }

    /// Every question asked so far, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }

    fn told(&self, what: String) {
        crate::noted(&self.asked, what);
    }
}

/// The candidate every question finds.
fn found(reference: &str) -> Candidate {
    Candidate {
        reference: reference.to_owned(),
        name: "Kind of Blue".to_owned(),
        by: None,
        year: None,
        cover: None,
        format: None,
        state: CandidateState::MayAsk,
    }
}

#[async_trait]
impl MusicCurating for Curator {
    async fn lookup(
        &self,
        unit: MusicUnit,
        term: &str,
        _: Option<&str>,
    ) -> Result<Candidates, Failure> {
        self.told(format!("lookup {unit:?} {term}"));
        Ok(Candidates {
            found: vec![found(term)],
            next: None,
        })
    }

    async fn candidate(
        &self,
        unit: MusicUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure> {
        self.told(format!("candidate {unit:?} {reference}"));
        Ok(Some(found(reference)))
    }

    async fn request(&self, ask: &MusicAsk) -> Result<Candidate, Failure> {
        self.told(format!(
            "request {:?} {} {} {} {} {}",
            ask.unit,
            ask.reference,
            ask.root_folder,
            ask.quality_profile,
            ask.metadata_profile,
            ask.search
        ));
        Ok(found(&ask.reference))
    }

    async fn profiles(&self) -> Result<AskedWith, Failure> {
        Ok(self.with.clone())
    }
}

#[async_trait]
impl BookCurating for Curator {
    async fn lookup(
        &self,
        unit: BookUnit,
        term: &str,
        _: Option<&str>,
    ) -> Result<Candidates, Failure> {
        self.told(format!("lookup {unit:?} {term}"));
        Ok(Candidates::default())
    }

    async fn candidate(
        &self,
        unit: BookUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure> {
        self.told(format!("candidate {unit:?} {reference}"));
        Ok(None)
    }

    async fn request(&self, ask: &BookAsk) -> Result<Candidate, Failure> {
        self.told(format!(
            "request {:?} {} {} {} {}",
            ask.unit, ask.reference, ask.root_folder, ask.quality_profile, ask.search
        ));
        Ok(found(&ask.reference))
    }

    async fn profiles(&self) -> Result<AskedWith, Failure> {
        Ok(self.with.clone())
    }
}

#[cfg(test)]
mod tests;

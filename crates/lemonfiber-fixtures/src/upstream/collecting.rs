//! The upstream as a curator of music and of books.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    AskedWith, BookAsk, BookCurating, BookUnit, Candidate, CandidateState, Candidates, Failure,
    MusicAsk, MusicCurating, MusicUnit, Profile,
};

use super::Upstream;

/// The reference the curator holds nothing under.
pub const UNHELD: &str = "unheld";

/// The one candidate every lookup finds: what `unit` names, filed in `format` where given.
fn found(unit: &str, format: Option<&str>) -> Candidate {
    Candidate {
        reference: format!("{unit}-0d1e"),
        name: "Harbour Lights".to_owned(),
        by: Some("The Lanterns".to_owned()),
        year: Some(2019),
        cover: Some("https://covers.example.invalid/harbour.jpg".to_owned()),
        format: format.map(str::to_owned),
        state: CandidateState::MayAsk,
    }
}

/// What a curator answers it may be asked with; a curator of books holds no metadata
/// profiles.
fn asked_with(metadata: bool) -> AskedWith {
    let standard = Profile {
        id: 1,
        name: "Standard".to_owned(),
    };
    AskedWith {
        units: if metadata {
            vec![MusicUnit::Album.into(), MusicUnit::Artist.into()]
        } else {
            vec![BookUnit::Book.into()]
        },
        root_folders: vec!["/data/media/music".to_owned()],
        quality: vec![standard.clone()],
        metadata: if metadata { vec![standard] } else { Vec::new() },
    }
}

/// What a curator refuses for a reference it does not hold.
fn unheld(reference: &str) -> Failure {
    Failure::Refused {
        service: "upstream".to_owned(),
        detail: format!("its catalogue holds no {reference}"),
    }
}

#[async_trait]
impl MusicCurating for Upstream {
    async fn lookup(
        &self,
        unit: MusicUnit,
        term: &str,
        after: Option<&str>,
    ) -> Result<Candidates, Failure> {
        self.tell(format!("lookup {unit:?} {term} {after:?}"));
        Ok(Candidates {
            found: vec![found(&format!("{unit:?}"), None)],
            next: after.is_none().then(|| "2".to_owned()),
        })
    }
    async fn candidate(
        &self,
        unit: MusicUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure> {
        self.tell(format!("candidate {unit:?} {reference}"));
        Ok((reference != UNHELD).then(|| found(&format!("{unit:?}"), None)))
    }
    async fn request(&self, ask: &MusicAsk) -> Result<Candidate, Failure> {
        self.tell(format!("request {ask:?}"));
        if ask.reference == UNHELD {
            return Err(unheld(&ask.reference));
        }
        Ok(Candidate {
            state: CandidateState::Asked,
            ..found(&format!("{:?}", ask.unit), None)
        })
    }
    async fn profiles(&self) -> Result<AskedWith, Failure> {
        self.tell("profiles".to_owned());
        Ok(asked_with(true))
    }
}

#[async_trait]
impl BookCurating for Upstream {
    async fn lookup(
        &self,
        unit: BookUnit,
        term: &str,
        after: Option<&str>,
    ) -> Result<Candidates, Failure> {
        self.tell(format!("lookup {unit:?} {term} {after:?}"));
        Ok(Candidates {
            found: vec![found(&format!("{unit:?}"), Some("ebook"))],
            next: None,
        })
    }
    async fn candidate(
        &self,
        unit: BookUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure> {
        self.tell(format!("candidate {unit:?} {reference}"));
        Ok((reference != UNHELD).then(|| found(&format!("{unit:?}"), Some("ebook"))))
    }
    async fn request(&self, ask: &BookAsk) -> Result<Candidate, Failure> {
        self.tell(format!("request {ask:?}"));
        if ask.reference == UNHELD {
            return Err(unheld(&ask.reference));
        }
        Ok(Candidate {
            state: CandidateState::Here,
            ..found(&format!("{:?}", ask.unit), Some("ebook"))
        })
    }
    async fn profiles(&self) -> Result<AskedWith, Failure> {
        self.tell("profiles".to_owned());
        Ok(asked_with(false))
    }
}

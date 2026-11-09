//! `music.curate` and `books.curate` across their contracts: one script run against a
//! curator in process and through each contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    AskedWith, BookAsk, BookCurating, BookUnit, Candidate, CandidateState, Candidates, Failure,
    MusicAsk, MusicCurating, MusicUnit, Profile,
};

use super::{books, crosses_alike, music, Upstream};

/// The reference the curator holds nothing under.
const UNHELD: &str = "unheld";

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

/// A result as the script writes it down: a failure by its kind and words alone.
fn said<T: std::fmt::Debug>(result: Result<T, Failure>) -> String {
    match result {
        Ok(answer) => format!("{answer:?}"),
        Err(Failure::Refused { detail, .. }) => format!("refused: {detail}"),
        Err(failure) => format!("{failure}"),
    }
}

/// Every operation of `music.curate`, each answer written down.
async fn listening<C: MusicCurating>(curator: &C) -> Vec<String> {
    let ask = |unit, reference: &str| MusicAsk {
        unit,
        reference: reference.to_owned(),
        root_folder: "/data/media/music".to_owned(),
        quality_profile: 1,
        metadata_profile: 1,
        search: true,
    };
    vec![
        said(curator.lookup(MusicUnit::Album, "harbour", None).await),
        said(
            curator
                .lookup(MusicUnit::Artist, "lanterns", Some("2"))
                .await,
        ),
        said(curator.candidate(MusicUnit::Album, "album-0d1e").await),
        said(curator.candidate(MusicUnit::Artist, UNHELD).await),
        said(curator.request(&ask(MusicUnit::Album, "album-0d1e")).await),
        said(curator.request(&ask(MusicUnit::Artist, UNHELD)).await),
        said(curator.profiles().await),
    ]
}

/// Every operation of `books.curate`, each answer written down.
async fn reading<C: BookCurating>(curator: &C) -> Vec<String> {
    let ask = |unit, reference: &str| BookAsk {
        unit,
        reference: reference.to_owned(),
        root_folder: "/data/media/books".to_owned(),
        quality_profile: 1,
        search: false,
    };
    vec![
        said(curator.lookup(BookUnit::Book, "harbour", None).await),
        said(curator.lookup(BookUnit::Series, "lights", Some("2")).await),
        said(curator.candidate(BookUnit::Series, "series-0d1e").await),
        said(curator.candidate(BookUnit::Book, UNHELD).await),
        said(curator.request(&ask(BookUnit::Author, "author-0d1e")).await),
        said(curator.request(&ask(BookUnit::Book, UNHELD)).await),
        said(curator.profiles().await),
    ]
}

#[tokio::test]
async fn a_curator_of_music_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        listening,
        music::curate::Adapter,
        [
            "lookup",
            "lookup",
            "candidate",
            "candidate",
            "request",
            "request",
            "profiles"
        ]
    );
}

#[tokio::test]
async fn a_curator_of_books_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        reading,
        books::curate::Adapter,
        [
            "lookup",
            "lookup",
            "candidate",
            "candidate",
            "request",
            "request",
            "profiles"
        ]
    );
}

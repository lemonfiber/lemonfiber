use std::sync::Mutex;

use async_trait::async_trait;

use super::Narrator;

#[derive(Default)]
struct Saying {
    said: Mutex<Vec<String>>,
}

#[async_trait]
impl Narrator for Saying {
    async fn say(&self, said: &str) {
        if let Ok(mut heard) = self.said.lock() {
            heard.push(said.to_owned());
        }
    }
}

impl Saying {
    fn said(&self) -> Vec<String> {
        self.said
            .lock()
            .map(|said| said.clone())
            .unwrap_or_default()
    }
}

#[tokio::test]
async fn a_narrator_that_takes_no_pull_lines_says_none_of_them() {
    let narrator = Saying::default();

    narrator.pulled("jellyfin Pulling").await;
    narrator.say("waiting").await;

    assert_eq!(narrator.said(), vec!["waiting".to_owned()]);
}

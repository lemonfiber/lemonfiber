use super::{Answer, Outcome};
use crate::app::exporting::Exported;
use crate::app::standing::StackStanding;
use crate::model::{kind, Octets};
use crate::notify::events::Heard;
use crate::screening::{Renewed, Untuned};

#[test]
fn each_answer_is_written_and_described_under_its_own_kind() {
    let answers = [
        (Answer::Standing(StackStanding::default()), kind::STANDING),
        (
            Answer::Heard(Heard {
                service: "sonarr".to_owned(),
            }),
            kind::NOTIFY_HEARD,
        ),
        (
            Answer::Renewed(Renewed {
                member: "a7f3".to_owned(),
                lasts_until: "2026-12-25".to_owned(),
                rehearsed: false,
            }),
            kind::RENEWED,
        ),
        (Answer::Untuned(Untuned { closed: 1 }), kind::UNTUNED),
        (
            Answer::Exported(Exported {
                path: "lemonfiber-logs-1-1.txt".to_owned(),
                rehearsed: false,
            }),
            kind::LOGS_EXPORTED,
        ),
        (
            Answer::Octets(Octets {
                media_type: "image/png",
                bytes: b"png".to_vec(),
            }),
            kind::OCTETS,
        ),
    ];
    for (answer, published) in answers {
        let outcome = Outcome::Surfaced(answer);
        assert_eq!(outcome.kind(), published);
        assert!(serde_json::to_value(&outcome).is_ok_and(|written| written.is_object()));
    }
    let mut described = Vec::new();
    Answer::schemas(&mut |described_as, _| described.push(described_as));
    assert_eq!(
        described,
        [
            kind::STANDING,
            kind::NOTIFY_HEARD,
            kind::RENEWED,
            kind::UNTUNED,
            kind::LOGS_EXPORTED
        ]
    );
}

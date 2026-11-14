use super::{PlayReport, TunedReport, WITHHELD};

/// Where a live stream or a title plays from carries its pass, so neither report shows it
/// where it is debugged, and each says one is there.
#[test]
fn no_report_shows_a_pass_where_it_is_debugged() {
    let pass = "pass-sealing-a-session";
    let at = || {
        Some(format!(
            "https://house.local:8920/live/ls1/stream.ts?pass={pass}"
        ))
    };
    let tuned = format!(
        "{:?}",
        TunedReport {
            channel: "kids".to_owned(),
            direct: at(),
            transcoded: at(),
            ..TunedReport::default()
        }
    );
    let played = format!(
        "{:?}",
        PlayReport {
            id: "0123456789abcdef0123456789abcdef".to_owned(),
            stream_from: at(),
            ..PlayReport::default()
        }
    );
    for debugged in [tuned, played] {
        assert!(!debugged.contains(pass), "{debugged}");
        assert!(debugged.contains(WITHHELD), "{debugged}");
    }
}

use super::Estimate;
use crate::quality::Preset;

/// A season is more than a film at the same quality, which is the whole of what
/// the figure is for.
#[test]
fn a_season_costs_more_than_a_film_at_the_same_quality() {
    assert!(Estimate::season(Preset::Balanced).bytes > Estimate::film(Preset::Balanced).bytes);
}

/// A higher preset costs more than a lower one for the same thing.
#[test]
fn a_higher_quality_costs_more_for_the_same_thing() {
    let mut last = 0;
    for preset in Preset::ALL {
        let estimated = Estimate::film(preset).bytes;
        assert!(
            estimated > last,
            "{preset:?} is not dearer than the one below"
        );
        last = estimated;
    }
}

/// The figure carries the hedge, so a surface cannot render it as a measurement.
#[test]
fn the_figure_carries_the_word_that_makes_it_a_guess() {
    let estimated = Estimate::season(Preset::Maximum);

    assert!(!estimated.measured, "nothing here has measured anything");
    assert!(estimated.reading().starts_with("about "), "{estimated:?}");
    assert!(estimated.reading().contains("GiB"), "{estimated:?}");
}

/// It serialises with the word beside the number, which is what a browser reads.
#[test]
fn it_serialises_with_the_word_beside_the_number() {
    let written = serde_json::to_string(&Estimate::film(Preset::SpaceSaving)).unwrap_or_default();

    assert!(written.contains("\"measured\":false"), "{written}");
    assert!(written.contains("\"bytes\":1500000000"), "{written}");
}

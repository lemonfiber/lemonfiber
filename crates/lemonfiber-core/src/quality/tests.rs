use super::{Consequence, Preset, Selection};

#[test]
fn every_preset_round_trips_through_its_plain_label() {
    for preset in Preset::ALL {
        assert_eq!(Preset::from_label(preset.label()), Some(preset));
    }
}

#[test]
fn an_unknown_label_is_refused_rather_than_guessed() {
    assert_eq!(Preset::from_label("ultra"), None);
    assert_eq!(Preset::from_label(""), None);
}

#[test]
fn the_default_is_balanced() {
    assert_eq!(Preset::default_preset(), Preset::Balanced);
}

#[test]
fn labels_reference_no_custom_format_or_scoring_term() {
    // The whole point of the feature: the operator never meets the tool's
    // vocabulary, only their own question about how good and how much disk.
    for preset in Preset::ALL {
        let label = preset.label();
        for term in ["format", "score", "profile", "trash", "custom"] {
            assert!(!label.contains(term), "{label} leaks the tool's vocabulary");
        }
    }
}

#[test]
fn each_preset_states_a_size_and_a_transcoding_consequence() {
    for preset in Preset::ALL {
        let Consequence {
            resolution,
            size_per_hour,
            transcoding,
        } = preset.consequence();
        assert!(!resolution.is_empty());
        assert!(
            size_per_hour.contains("GB"),
            "{size_per_hour} names no size"
        );
        assert!(!transcoding.is_empty());
        assert!(!preset.means().is_empty());
    }
}

#[test]
fn only_maximum_calls_out_transcoding_as_a_likely_cost() {
    // The transcoding warning the platform check later gates on is meaningful only
    // for the 4K HDR preset — it is the one that "often needs" transcoding; the
    // 1080p tiers direct-play, at most a weak client "may" transcode a high one.
    for preset in Preset::ALL {
        let likely = preset
            .consequence()
            .transcoding
            .contains("often needs transcod");
        assert_eq!(
            likely,
            preset == Preset::Maximum,
            "{preset:?} misstates whether transcoding is a likely cost",
        );
        // The plain boolean the platform check gates on must agree with the
        // prose, so the two can never drift apart.
        assert_eq!(
            preset.likely_needs_transcoding(),
            likely,
            "{preset:?} boolean and prose disagree on transcoding",
        );
    }
}

#[test]
fn a_hungrier_preset_costs_more_disk_per_hour() {
    // The projection turns on the ratio between presets, so the rate must rise
    // strictly with how demanding the preset is, in the order they are offered.
    let mut previous = 0;
    for preset in Preset::ALL {
        let rate = preset.bytes_per_hour();
        assert!(
            rate > previous,
            "{preset:?} must cost more per hour than the tier below it"
        );
        previous = rate;
    }
}

#[test]
fn the_most_demanding_choice_is_the_hungriest_preset_in_force() {
    let mut selection = Selection::everywhere(Preset::SpaceSaving);
    assert_eq!(selection.most_demanding(), Preset::SpaceSaving);

    // A hungrier per-type exception raises the basis; a lighter one does not.
    selection.set_type("movies", Preset::Maximum);
    selection.set_type("tv", Preset::Balanced);
    assert_eq!(selection.most_demanding(), Preset::Maximum);
}

#[test]
fn a_type_takes_the_global_preset_until_given_its_own() {
    let mut selection = Selection::everywhere(Preset::Balanced);
    assert_eq!(selection.for_type("tv"), Preset::Balanced);
    assert!(!selection.is_overridden());

    selection.set_type("movies", Preset::Maximum);
    assert_eq!(selection.for_type("movies"), Preset::Maximum);
    assert_eq!(selection.for_type("tv"), Preset::Balanced);
    assert!(selection.is_overridden());
}

#[test]
fn overrides_reports_the_global_and_only_the_genuine_exceptions() {
    let mut selection = Selection::everywhere(Preset::Balanced);
    assert_eq!(selection.global(), Preset::Balanced);
    assert_eq!(selection.overrides().count(), 0);

    selection.set_type("movies", Preset::Maximum);
    let overrides: Vec<_> = selection.overrides().collect();
    assert_eq!(overrides, vec![("movies", Preset::Maximum)]);
}

#[test]
fn raising_the_global_drops_an_exception_it_now_matches() {
    let mut selection = Selection::everywhere(Preset::Balanced);
    selection.set_type("movies", Preset::Maximum);
    selection.set_type("tv", Preset::HighQuality);

    selection.set_global(Preset::Maximum);
    assert_eq!(selection.global(), Preset::Maximum);
    // Movies matched the new global, so it is no longer an exception; tv still
    // differs and remains one.
    assert_eq!(
        selection.overrides().collect::<Vec<_>>(),
        vec![("tv", Preset::HighQuality)]
    );
}

#[test]
fn setting_a_type_back_to_the_global_choice_is_not_an_override() {
    let mut selection = Selection::everywhere(Preset::Balanced);
    selection.set_type("movies", Preset::Maximum);
    selection.set_type("movies", Preset::Balanced);
    assert_eq!(selection.for_type("movies"), Preset::Balanced);
    assert!(
        !selection.is_overridden(),
        "a type matching the global choice is not an exception"
    );
}

#[test]
fn a_redundant_deserialized_override_is_not_read_as_an_exception() {
    // Deserialization writes the map directly, so it can carry a per-type entry
    // equal to the global that `set_type` would have pruned. The `overridden`
    // state is whether a type differs, not whether the map is non-empty.
    let redundant = r#"{"global":"balanced","per-type":{"movies":"balanced"}}"#;
    let selection: Option<Selection> = serde_json::from_str(redundant).ok();
    assert!(selection.is_some_and(|selection| !selection.is_overridden()));
}

#[test]
fn a_selection_round_trips_through_its_serialised_form() {
    let mut selection = Selection::everywhere(Preset::HighQuality);
    selection.set_type("tv", Preset::SpaceSaving);
    let json = serde_json::to_string(&selection).unwrap_or_default();
    assert_eq!(
        serde_json::from_str::<Selection>(&json).ok(),
        Some(selection)
    );
}

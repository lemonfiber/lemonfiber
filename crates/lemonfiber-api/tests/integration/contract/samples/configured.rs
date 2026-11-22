//! The configuration as the `config` read answers it: no setting, a decision and the areas
//! left alone.

use lemonfiber_core::model::{ConfigReport, CoveredReport, DecisionReport, UnmanagedReport};

/// A listing holding no setting, one decision and the areas declared unmanaged.
pub(super) fn the_configuration() -> ConfigReport {
    ConfigReport {
        decisions: vec![a_decision()],
        unmanaged: areas_left_alone(),
        unmanaged_more: 0,
        settings: Vec::new(),
        changed: false,
        rehearsed: false,
        consequence: None,
        review: None,
    }
}

/// Areas declared unmanaged: one covering a thing of each kind, and a misspelt one
/// covering nothing.
fn areas_left_alone() -> Vec<UnmanagedReport> {
    use lemonfiber_core::unmanaged::CoveredKind;
    [
        (
            "sonarr",
            "I tune this one by hand",
            Some((CoveredKind::Service, "sonarr")),
        ),
        (
            "config/recyclarr",
            "my own profiles live in here",
            Some((CoveredKind::Path, "config/recyclarr/recyclarr.yml")),
        ),
        (
            "DATA_ROOT",
            "the disks are mine to move",
            Some((CoveredKind::Setting, "DATA_ROOT")),
        ),
        ("radar", "I tune this one by hand too", None),
    ]
    .into_iter()
    .map(|(area, why, covered)| UnmanagedReport {
        area: area.to_owned(),
        area_cut: false,
        why: why.to_owned(),
        why_cut: false,
        covers: covered
            .into_iter()
            .map(|(kind, name)| CoveredReport {
                kind,
                name: name.to_owned(),
            })
            .collect(),
        covers_more: 0,
    })
    .collect()
}

/// A decision the settings store keeps, every optional half filled.
fn a_decision() -> DecisionReport {
    use lemonfiber_core::reconfigure::{decision, Cost, DecisionKey};
    let key = DecisionKey::VpnProvider;
    let row = decision(key.as_str());
    DecisionReport {
        key,
        cost: row.map_or(Cost::Cheap, |row| row.cost),
        affects: row.map(|row| row.affects.to_owned()).unwrap_or_default(),
        held: true,
        credential: false,
        setting: Some("gluetun:provider".to_owned()),
    }
}

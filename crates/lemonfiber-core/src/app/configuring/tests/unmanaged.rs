use super::{ctx_holding, env_at, get, set, Setting};
use crate::app::{Ctx, Waiting};
use crate::model::UnmanagedReport;
use crate::stack::Source;
use crate::test_support::Holds as _;
use crate::unmanaged::CoveredKind;

/// The areas the `config` read answered at `ctx`.
async fn listed(ctx: &Ctx) -> Vec<UnmanagedReport> {
    get(ctx, None)
        .await
        .map(|report| report.unmanaged)
        .unwrap_or_default()
}

/// What each area covers, as kind and name.
fn covering(areas: &[UnmanagedReport]) -> Vec<(String, Vec<(CoveredKind, String)>)> {
    areas
        .iter()
        .map(|one| {
            (
                one.area.clone(),
                one.covers
                    .iter()
                    .map(|covered| (covered.kind, covered.name.clone()))
                    .collect(),
            )
        })
        .collect()
}

#[tokio::test]
async fn nothing_declared_is_an_empty_list_rather_than_none() {
    let ctx = ctx_holding(env_at("unmanaged-none", "DATA_ROOT=/srv\n"));
    let read = get(&ctx, None).await;
    assert_eq!(
        read.as_ref().map(|report| report.unmanaged.len()).ok(),
        Some(0)
    );
    assert_eq!(read.map(|report| report.unmanaged_more).ok(), Some(0));
}

#[tokio::test]
async fn each_area_is_listed_in_the_order_written_and_an_entry_that_declares_nothing_is_not() {
    let ctx = ctx_holding(env_at(
        "unmanaged-order",
        "LEMONFIBER_UNMANAGED=sonarr=I tune this by hand,=no area,radar=,radarr=mine too\n",
    ));
    let read: Vec<(String, String)> = listed(&ctx)
        .await
        .into_iter()
        .map(|one| (one.area, one.why))
        .collect();
    assert_eq!(
        read,
        [
            ("sonarr".to_owned(), "I tune this by hand".to_owned()),
            ("radarr".to_owned(), "mine too".to_owned())
        ]
    );
}

#[tokio::test]
async fn an_area_covers_the_services_settings_and_project_files_the_core_would_leave_alone() {
    let ctx = ctx_holding(env_at("unmanaged-covers", "DATA_ROOT=/srv\n"));
    let service = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .and_then(|manifest| manifest.services.into_iter().next())
        .map(|service| service.id)
        .unwrap_or_default();
    assert!(!service.is_empty(), "the bundled stack runs a service");
    let ctx = ctx_holding(env_at(
        "unmanaged-covers-declared",
        &format!(
            "DATA_ROOT=/srv\nLEMONFIBER_UNMANAGED={service}=by hand,DATA_ROOT=the disks are mine,\
             compose.generated.yml=my own edits,sonar=a typo\n"
        ),
    ));
    let read = covering(&listed(&ctx).await);
    assert_eq!(
        read,
        [
            (
                service.clone(),
                vec![(CoveredKind::Service, service.clone())]
            ),
            (
                "DATA_ROOT".to_owned(),
                vec![(CoveredKind::Setting, "DATA_ROOT".to_owned())]
            ),
            (
                "compose.generated.yml".to_owned(),
                vec![(CoveredKind::Path, crate::project::FILE.to_owned())]
            ),
            ("sonar".to_owned(), Vec::new()),
        ],
        "a misspelt area covers nothing"
    );
}

#[tokio::test]
async fn an_area_covers_each_file_the_stack_carries_beneath_it_and_none_beside_it() {
    let env = env_at(
        "unmanaged-carried",
        "LEMONFIBER_UNMANAGED=config=my own profiles live in here\n",
    );
    let ctx = crate::test_support::filed(
        crate::test_support::a_context()
            .over(Source::Embedded(&crate::test_support::STACKLET))
            .settings(crate::config::Settings {
                env_file: Some(env.to_path_buf()),
                ..crate::config::Settings::default()
            })
            .build(),
    )
    .holding(env);
    assert_eq!(
        covering(&listed(&ctx).await),
        [(
            "config".to_owned(),
            vec![(
                CoveredKind::Path,
                "config/recyclarr/recyclarr.yml".to_owned()
            )]
        )]
    );
}

#[tokio::test]
async fn a_setting_no_configuration_holds_is_covered_where_a_decision_names_it() {
    let ctx = ctx_holding(env_at(
        "unmanaged-decision",
        "LEMONFIBER_UNMANAGED=PUID=the user is mine\n",
    ));
    assert_eq!(
        covering(&listed(&ctx).await),
        [(
            "PUID".to_owned(),
            vec![(CoveredKind::Setting, "PUID".to_owned())]
        )]
    );
}

#[tokio::test]
async fn a_change_to_the_declarations_answers_the_list_as_it_stands_after_it() {
    let ctx = ctx_holding(env_at(
        "unmanaged-rehearsed",
        "DATA_ROOT=/srv\nLEMONFIBER_UNMANAGED=sonarr=by hand\n",
    ));
    let answer = set(
        &ctx,
        Setting {
            key: crate::config::UNMANAGED_KEY.to_owned(),
            value: "DATA_ROOT=the disks are mine".to_owned(),
            confirmed: false,
            waiting: Waiting::Never,
        },
    )
    .await;
    let areas: Vec<String> = answer
        .map(|report| report.unmanaged.into_iter().map(|one| one.area).collect())
        .unwrap_or_default();
    assert_eq!(areas, ["DATA_ROOT"]);
}

use super::super::super::register::{Register, Unreadable};
use super::{each, installed, placed, set, whole};
use crate::plugin::{Installed, Placed};

#[test]
fn a_service_shows_the_dashboard_line_and_icon_its_wiring_names_or_its_own() {
    let named = installed(|manifest| {
        for wiring in &mut manifest.wirings {
            wiring.dashboard_description = Some("Comics".to_owned());
            wiring.dashboard_icon = Some("mdi-book-open".to_owned());
        }
    });
    let one = placed(named.as_ref(), "komga");
    assert_eq!(
        one.as_ref().map(|one| one.description.as_str()),
        Some("Comics")
    );
    assert_eq!(
        one.as_ref().map(Placed::icon).as_deref(),
        Some("mdi-book-open")
    );

    let own = placed(whole().as_ref(), "komga");
    assert_eq!(
        own.as_ref().map(|one| one.description.as_str()),
        Some("Reads your comics on any browser")
    );
    assert_eq!(own.as_ref().map(Placed::icon).as_deref(), Some("komga.png"));
}

#[test]
fn only_a_health_asked_over_http_keeps_its_path() {
    let health = |kind, path: Option<&str>| {
        installed(|manifest| {
            set(manifest, |service| {
                service.health = Some(lemonfiber_plugin::Health {
                    kind,
                    path: path.map(str::to_owned),
                    timeout_s: None,
                });
            });
        })
    };
    let kept =
        |record: Option<Installed>| placed(record.as_ref(), "komga").and_then(|one| one.health);

    assert_eq!(
        kept(health(lemonfiber_plugin::HealthKind::Http, Some("/ping"))),
        Some("/ping".to_owned())
    );
    assert_eq!(
        kept(health(lemonfiber_plugin::HealthKind::Tcp, Some("/ping"))),
        None
    );
}

#[test]
fn a_record_carrying_a_dashboard_value_this_build_would_not_have_written_is_refused() {
    let altered: [(&str, Change); 2] = [
        ("a health path that would name another host", |placed| {
            placed.health = Some("@elsewhere/health".to_owned());
        }),
        ("an icon the page would fetch from elsewhere", |placed| {
            placed.icon = Some("https://example.com/komga.png".to_owned());
        }),
    ];
    for (what, alter) in altered {
        let Some(mut one) = whole() else {
            unreachable!("the fixture reads");
        };
        each(&mut one, alter);
        let mut register = Register::empty();
        assert_eq!(register.record(one), Ok(()), "{what}");
        let read = Register::parse(&register.to_json().unwrap_or_default());
        assert!(
            matches!(&read, Err(Unreadable::Damaged(why)) if why.contains("would not have written")),
            "{what}: {read:?}"
        );
    }
}

/// One change made to every service of a record, for the case above.
type Change = fn(&mut Placed);

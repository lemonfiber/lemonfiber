use std::os::unix::fs::PermissionsExt as _;

use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_manifest::container::Certificate;

use super::{declaring, kept_or_made, served_with, throwaway, unmade};
use crate::certificate::Unkept;

fn laid() -> Certificate {
    Certificate {
        key: "tls/key.pem".to_owned(),
        certificate: "tls/certificate.pem".to_owned(),
        names: vec!["gateway".to_owned(), "gateway.home".to_owned()],
    }
}

fn mode(at: &std::path::Path) -> Option<u32> {
    std::fs::metadata(at)
        .ok()
        .map(|file| file.permissions().mode() & 0o777)
}

/// The names the certificate written at `at` is made out to.
fn named(at: &std::path::Path) -> Vec<String> {
    let text = std::fs::read_to_string(at).unwrap_or_default();
    let der = super::super::der(&text, "CERTIFICATE").unwrap_or_default();
    x509_parser::parse_x509_certificate(&der)
        .ok()
        .and_then(|(_, parsed)| {
            parsed
                .subject_alternative_name()
                .ok()
                .flatten()
                .map(|names| {
                    names
                        .value
                        .general_names
                        .iter()
                        .filter_map(|name| match name {
                            x509_parser::extensions::GeneralName::DNSName(dns) => {
                                Some((*dns).to_owned())
                            }
                            _ => None,
                        })
                        .collect()
                })
        })
        .unwrap_or_default()
}

#[test]
fn a_declared_certificate_is_made_once_where_it_says_for_its_names_and_kept() {
    let at = Scratch::new("declared-kept");
    let made = kept_or_made(&lemonfiber_adapters::Disk, &at, &laid());
    let again = kept_or_made(&lemonfiber_adapters::Disk, &at, &laid());
    assert!(made.is_ok(), "{made:?}");
    assert_eq!(made, again);
    assert_eq!(mode(&at.join("tls/key.pem")), Some(0o600));
    assert_eq!(mode(&at.join("tls/certificate.pem")), Some(0o600));
    assert_eq!(
        named(&at.join("tls/certificate.pem")),
        vec!["gateway", "gateway.home"]
    );
}

#[test]
fn half_a_pair_is_refused_naming_both_and_never_made_again_over() {
    let at = Scratch::new("declared-half");
    let _ = kept_or_made(&lemonfiber_adapters::Disk, &at, &laid());
    let _ = std::fs::remove_file(at.join("tls/certificate.pem"));
    let refused = kept_or_made(&lemonfiber_adapters::Disk, &at, &laid());
    assert!(
        matches!(&refused, Err(Unkept::Unreadable(why))
            if why.contains("tls/certificate.pem") && why.contains("tls/key.pem")),
        "{refused:?}"
    );
    assert!(!at.join("tls/certificate.pem").exists());
}

#[cfg(unix)]
#[test]
fn a_link_where_the_key_goes_is_never_followed() {
    let at = Scratch::new("declared-linked");
    let elsewhere = Scratch::new("declared-linked-elsewhere");
    let _ = std::fs::create_dir_all(at.join("tls"));
    let _ = std::os::unix::fs::symlink(elsewhere.join("stolen.pem"), at.join("tls/key.pem"));
    let made = throwaway(&lemonfiber_adapters::Disk, &at, &laid());
    assert!(made.is_ok(), "{made:?}");
    assert!(!elsewhere.join("stolen.pem").exists());
    assert!(std::fs::symlink_metadata(at.join("tls/key.pem")).is_ok_and(|meta| meta.is_file()));
}

#[test]
fn a_stream_guard_serves_with_the_doors_own_and_another_with_what_it_declares() {
    let guard = served_with(&["stream.guard".to_owned()], None);
    assert_eq!(guard, Some(super::super::own()));
    assert_eq!(served_with(&[], Some(&laid())), Some(laid()));
    assert_eq!(served_with(&["media.serve".to_owned()], None), None);
}

#[test]
fn every_service_declaring_one_is_made_for_beneath_its_configuration_directory() {
    let mut own = crate::door::fixtures::service("gateway", None);
    own.certificate = Some(laid());
    let plain = crate::door::fixtures::service("watcher", None);
    let mut brought = crate::door::fixtures::brought("relay", None);
    brought.fields.certificate = Some(laid());
    let unbrought = crate::door::fixtures::brought("quiet", None);
    let project = std::path::Path::new("/stack");
    let declared = declaring(project, &[own, plain], [&brought, &unbrought].into_iter());
    assert_eq!(
        declared,
        vec![
            ("gateway".to_owned(), project.join("config/gateway"), laid()),
            ("relay".to_owned(), project.join("config/relay"), laid()),
        ]
    );
}

#[test]
fn a_certificate_not_made_refuses_the_start_naming_the_service_and_never_the_key() {
    let problem = unmade(
        "gateway",
        &Unkept::Unmade("tls/key.pem could not be written".to_owned()),
    );
    assert!(problem.summary.contains("gateway"), "{}", problem.summary);
    assert_eq!(
        problem.detail.as_deref(),
        Some("tls/key.pem could not be written")
    );
}

/// A context whose stack is written at its own directory, with `komga` installed declaring
/// the certificate [`laid`] gives, and that directory.
fn installed(name: &str, declared: Certificate) -> (crate::app::Ctx, std::path::PathBuf) {
    let scratch = Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let env = project.join(".env");
    let mut placed = crate::test_support::a_placed("komga", &[], None);
    placed.fields.certificate = Some(declared);
    let mut register = crate::plugin::Register::empty();
    assert!(register
        .record(crate::test_support::an_installed("komga", vec![placed]))
        .is_ok());
    assert!(std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        register.to_json().unwrap_or_default()
    )
    .is_ok());
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build();
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(env);
    (ctx, project)
}

#[test]
fn an_installed_plugins_declared_certificate_is_made_before_the_stack_starts() {
    let (ctx, project) = installed("declared-before-start", laid());
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the stack reads");
    };
    assert!(super::made_before_start(&ctx, &manifest).is_ok());
    let config = project.join("config/komga");
    let kept = crate::certificate::kept_as(&lemonfiber_adapters::Disk, &config, &laid());
    assert!(kept.as_ref().is_ok_and(Option::is_some), "{kept:?}");
}

#[test]
fn a_start_whose_certificate_is_half_there_is_refused_naming_the_service() {
    let (ctx, project) = installed("declared-half-at-start", laid());
    let config = project.join("config/komga/tls");
    let _ = std::fs::create_dir_all(&config);
    let _ = std::fs::write(config.join("key.pem"), "not a key");
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the stack reads");
    };
    let refused = super::made_before_start(&ctx, &manifest);
    assert!(
        refused.as_ref().is_err_and(|problem| problem.code
            == crate::error::codes::stack::CERTIFICATE_NOT_MADE
            && problem.summary.contains("komga")),
        "{refused:?}"
    );
}

#[test]
fn nothing_is_made_where_no_stack_directory_is_known() {
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build();
    ctx.settings.stack_dir = None;
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the stack reads");
    };
    assert!(super::made_before_start(&ctx, &manifest).is_ok());
}

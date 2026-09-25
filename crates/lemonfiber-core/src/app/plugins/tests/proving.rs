//! Starting a plugin's service and asking its proofs before it is kept.

use super::*;

/// What the proofs on a report came to, in the order they were declared.
fn verdicts(outcome: Result<Installs, Box<crate::error::Problem>>) -> Vec<Option<Verdict>> {
    report(outcome)
        .and_then(|one| one.install)
        .map(|one| one.proofs.into_iter().map(|proof| proof.came_to).collect())
        .unwrap_or_default()
}

/// A refusal's own code and the code of whatever it carries underneath it.
fn beneath(outcome: Result<Installs, Box<crate::error::Problem>>) -> (String, String) {
    outcome
        .err()
        .map(|problem| {
            (
                problem.code.to_string(),
                problem
                    .cause
                    .map(|cause| cause.code.to_string())
                    .unwrap_or_default(),
            )
        })
        .unwrap_or_default()
}

/// The whole of what a proof buys: the plugin's own service is started, asked what
/// the manifest said it would answer, and recorded as installed only once it has
/// answered it.
#[tokio::test]
async fn a_plugin_whose_proof_holds_is_started_asked_and_then_recorded() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("proved", runner.clone(), answering(200));

    assert_eq!(
        counted(installing(&ctx, &source("proved", PROVING)).await),
        Some(1)
    );
    assert!(runner.ran("up"), "its own service was started");
    assert!(
        !runner.ran("rm"),
        "and nothing was taken back off the machine"
    );
}

impl LostToTheInstall {
    /// One of them, before anything has started.
    pub(super) fn losing(once_up: &'static str) -> Arc<Self> {
        Arc::new(Self {
            once_up,
            started: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

#[async_trait::async_trait]
impl crate::ports::Runner for LostToTheInstall {
    async fn run(
        &self,
        argv: &[String],
    ) -> Result<lemonfiber_ports::process::Output, lemonfiber_ports::process::Failure> {
        let said = |named: &str| argv.iter().any(|word| word == named);
        if said("up") {
            self.started
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if said("version")
            && said(self.once_up)
            && self.started.load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(lemonfiber_ports::process::Failure::NotFound {
                program: "docker".to_owned(),
            });
        }
        Ok(spoke("25.0.5|1.44|1.44"))
    }
}

/// The whole of what the second half of an install buys. The plugin's own proofs
/// held — its service answered exactly what the manifest said it would — and the
/// stack was still taken back, because a check that was passing before the install
/// is failing after it.
///
/// **A differential attributes by time rather than by cause, and that is
/// deliberate.** Nothing here can prove the install is what broke the engine check,
/// and neither can an operator's machine. What it can say is that the check held
/// before and does not now, which is the honest claim and the one worth acting on.
#[tokio::test]
async fn a_plugin_that_holds_its_own_proofs_and_breaks_the_stack_is_still_put_back() {
    let ctx = proving(
        "collateral",
        LostToTheInstall::losing("version"),
        answering(200),
    );

    let install =
        report(installing(&ctx, &source("collateral", PROVING)).await).and_then(|one| one.install);
    assert_eq!(
        install
            .as_ref()
            .map(|one| came_to(one.proofs.first().and_then(|proof| proof.came_to.as_ref()))),
        Some("held"),
        "the plugin answered its own proof"
    );
    assert_eq!(
        install.as_ref().map(|one| one.recorded),
        Some(false),
        "and is not recorded as installed"
    );
    let broke: Vec<String> = install
        .as_ref()
        .and_then(|one| one.verified.as_ref())
        .map(|checked| {
            checked
                .broke
                .iter()
                .map(|one| one.now.check.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        broke.iter().any(|check| check == "environment.engine"),
        "the check that changed is named: {broke:?}"
    );
    assert!(
        install
            .as_ref()
            .and_then(|one| one.reversed.as_ref())
            .is_some(),
        "and the install went back"
    );
    assert!(
        !stack_of(&ctx).join("compose/plugins/komga.yml").exists(),
        "so what it wrote is gone again"
    );
}

/// A check that could no longer be told either way is reported and the install
/// still stands. *I could not tell* is not *it is still broken*, and an install
/// reversed on it would be punishing a plugin for something nobody established.
#[tokio::test]
async fn a_check_nothing_could_establish_is_reported_rather_than_held_against_it() {
    let ctx = proving(
        "unsettled",
        LostToTheInstall::losing("compose"),
        answering(200),
    );

    let install =
        report(installing(&ctx, &source("unsettled", PROVING)).await).and_then(|one| one.install);
    let checked = install.as_ref().and_then(|one| one.verified.as_ref());
    assert!(
        checked.is_some_and(|one| one
            .unsettled
            .iter()
            .any(|changed| changed.now.check == "environment.compose")),
        "what could not be told is said out loud"
    );
    assert!(
        checked.is_some_and(crate::plugin::Verification::held),
        "and it does not stop the install"
    );
    assert_eq!(
        install.as_ref().map(|one| one.recorded),
        Some(true),
        "which is to say the plugin is installed"
    );
}

/// An install the stack was fine with says so, rather than saying nothing. A run
/// that reported no verification at all would leave a reader unable to tell a
/// clean reading from a reading nobody took.
#[tokio::test]
async fn an_install_the_stack_was_fine_with_says_the_checks_found_nothing() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("unbroken", runner, answering(200));

    let checked = report(installing(&ctx, &source("unbroken", PROVING)).await)
        .and_then(|one| one.install)
        .and_then(|one| one.verified);
    assert!(
        checked
            .as_ref()
            .is_some_and(crate::plugin::Verification::held),
        "the checks were taken and nothing was worse for it"
    );
    assert!(
        checked.is_some_and(|one| one.broke.is_empty() && one.unsettled.is_empty()),
        "on a machine every fake answers the same way twice"
    );
}

/// A rehearsal takes no reading at all, and says nothing about one. A heading
/// saying the checks found nothing would be a claim about a reading that never
/// happened.
#[tokio::test]
async fn a_rehearsal_reports_no_verification_because_it_took_none() {
    let ctx = rehearsing("unchecked");

    assert!(
        report(installing(&ctx, &source("unchecked", PROVING)).await)
            .and_then(|one| one.install)
            .is_some_and(|one| one.verified.is_none())
    );
}

/// **A plugin's own contributed rows do not gate its own install, deliberately.**
/// The register is written last, so the second reading of the stack's checks does
/// not hold this plugin's rows — and it must not. What a contributed row says is
/// an ongoing fact about a service an operator is running; a freshly installed one
/// very often has nothing in it yet, and an install reversed for that would refuse
/// every plugin whose first row is *is there anything in here*. What gates the
/// install is what the plugin declared as a proof, which is the field that exists
/// for saying so.
#[tokio::test]
async fn a_plugins_own_contributed_row_does_not_gate_its_own_install() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving(
        "contributing",
        runner,
        Fake::by_path(vec![
            (
                "/api/v1/libraries",
                lemonfiber_fixtures::http::Answer::reply(200, "[]"),
            ),
            (
                "/api/v1/claim",
                lemonfiber_fixtures::http::Answer::reply(500, "no"),
            ),
        ]),
    );

    let install = report(installing(&ctx, &source("contributing", CONTRIBUTING)).await)
        .and_then(|one| one.install);
    assert_eq!(
        install.as_ref().map(|one| one.recorded),
        Some(true),
        "the plugin holds its own proof and is installed"
    );
    assert!(
        install
            .as_ref()
            .and_then(|one| one.verified.as_ref())
            .is_some_and(crate::plugin::Verification::held),
        "and the row it contributes is not among what the stack was asked"
    );
    assert!(
        install.is_some_and(|one| one
            .would
            .contributions
            .iter()
            .any(|row| row.id == "komga:claimed" && row.service.as_deref() == Some("komga"))),
        "though the record keeps it, with the service it asks already settled"
    );
}

/// The verdict is on the report, and so is what it was reached against — because a
/// verdict against a recording the author shipped and one against the service on
/// this machine are not the same claim, and a reader handed one has nothing else in
/// the document to tell them apart.
#[tokio::test]
async fn the_report_carries_the_verdict_and_says_it_was_the_service_that_answered() {
    let ctx = proving(
        "against",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    let shown =
        report(installing(&ctx, &source("against", PROVING)).await).and_then(|one| one.install);

    assert_eq!(
        shown.as_ref().and_then(|one| one.against),
        Some(crate::plugin::Evidence::Service)
    );
    let stated: Vec<Option<Verdict>> = shown
        .map(|one| one.proofs.into_iter().map(|proof| proof.came_to).collect())
        .unwrap_or_default();
    assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "held");
    assert!(
        why(stated.first().and_then(Option::as_ref)).is_empty(),
        "a proof that held carries no reason, because nothing stopped it"
    );
}

/// A proof the service refuses stops the install, and the install goes back: the
/// container comes off the machine and every file it wrote is removed.
#[tokio::test]
async fn a_proof_the_service_refuses_puts_the_whole_install_back() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("refuted", runner.clone(), answering(503));

    let outcome = installing(&ctx, &source("refuted", PROVING)).await;
    assert_eq!(
        came_to(verdicts(outcome).first().and_then(Option::as_ref)),
        "failed"
    );

    assert!(runner.ran("rm"), "its container came off the machine");
    assert!(
        !stack_of(&ctx).join("compose/plugins/komga.yml").exists(),
        "and the document it wrote is gone"
    );
    assert_eq!(
        counted(reading(&ctx).await),
        Some(0),
        "and nothing is installed"
    );
}

/// The reversal is the rollback layer's, reported in the rollback layer's own
/// shape: what went back, and what did not with the reason each is standing.
#[tokio::test]
async fn what_a_failed_install_put_back_is_reported_as_the_reversal_it_was() {
    let ctx = proving(
        "reversed",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(503),
    );
    let put_back = report(installing(&ctx, &source("reversed", PROVING)).await)
        .and_then(|one| one.install)
        .and_then(|one| one.reversed);

    let put_back = put_back.unwrap_or_default();
    assert!(!put_back.reversed.is_empty(), "what went back is named");
    assert!(put_back.left.is_empty(), "and nothing was left standing");
    assert!(put_back
        .reversed
        .iter()
        .any(|undo| undo.target.ends_with("komga.yml")));
}

/// A service that never answers is unproven rather than refuted, and the install
/// goes back all the same. What it is called and what it costs are two decisions:
/// nothing answered, so nothing was established, and installing over that would
/// report an install as complete on the strength of a question nobody answered.
#[tokio::test]
async fn a_service_that_never_answers_is_unproven_and_the_install_still_goes_back() {
    let ctx = proving(
        "silent",
        Arc::new(Recording::answering(Ok(spoke("")))),
        Fake::silent(),
    );

    let outcome = installing(&ctx, &source("silent", PROVING)).await;
    assert_eq!(
        came_to(verdicts(outcome).first().and_then(Option::as_ref)),
        "unproven"
    );
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// A service that has not answered *yet* is asked again. A container Compose has
/// just created is not a service that is listening, and an install that took the
/// first refusal would fail on every image that takes a moment to open its socket.
#[tokio::test(start_paused = true)]
async fn a_service_that_has_not_answered_yet_is_asked_again() {
    let http = Fake::by_path_in_turn(vec![(
        "/api/v1/libraries",
        vec![
            lemonfiber_fixtures::http::Answer::Silent,
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ],
    )]);
    let mut ctx = proving(
        "patient",
        Arc::new(Recording::answering(Ok(spoke("")))),
        http.clone(),
    );
    ctx.patience = std::time::Duration::from_secs(30);

    assert_eq!(
        counted(installing(&ctx, &source("patient", PROVING)).await),
        Some(1)
    );
    let asked = http
        .requests()
        .into_iter()
        .filter(|request| request.url.contains("/api/v1/libraries"))
        .count();
    assert_eq!(asked, 2, "the proof was asked twice");
}

/// A proof of a service that publishes no port is unproven naming it, because
/// there is nowhere to ask rather than somewhere to guess.
#[tokio::test]
async fn a_proof_of_a_service_with_no_port_has_nowhere_to_ask() {
    let unpublished = PROVING.replace("port        = 25600\n", "");
    let unpublished = unpublished.replace("bind        = \"lan\"\n", "");
    let ctx = proving(
        "unpublished",
        Arc::new(Recording::answering(Ok(spoke("")))),
        Fake::silent(),
    );

    let stated = verdicts(installing(&ctx, &source("unpublished", &unpublished)).await);
    assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unproven");
    assert!(why(stated.first().and_then(Option::as_ref)).contains("publishes no port"));
}

/// A container that will not start stops the install by name, and the files it had
/// already written go back.
#[tokio::test]
async fn a_service_that_will_not_start_stops_the_install_and_the_files_go_back() {
    let runner = Keyed::answering(
        vec![("up", Ok(engine_refused("no such image")))],
        Ok(spoke("")),
    );
    let ctx = proving("unstartable", runner, Fake::silent());
    let at = source("unstartable", PROVING);

    let (code, said) = refused(installing(&ctx, &at).await);
    assert_eq!(code, "PLUGIN-9");
    assert!(
        said.contains("was put back") && !said.contains("Still standing"),
        "a reversal that finished says so and names nothing as standing: {said}"
    );
    assert!(
        made_paths(&ctx)
            .iter()
            .all(|path| !std::path::Path::new(path).exists()),
        "every path this run made is gone"
    );
}

/// A reversal that could not take the container off says what is still standing,
/// rather than repeating the sentence a reversal that finished would have got.
#[tokio::test]
async fn a_service_that_will_not_start_and_will_not_come_off_names_what_stands() {
    let runner = Keyed::answering(
        vec![
            ("up", Ok(engine_refused("no such image"))),
            ("rm", Ok(engine_refused("no such container"))),
        ],
        Ok(spoke("")),
    );
    let ctx = proving("standing", runner, Fake::silent());

    let (code, said) = refused(installing(&ctx, &source("standing", PROVING)).await);
    assert_eq!(code, "PLUGIN-9");
    assert!(
        said.contains("Still standing: komga"),
        "the container nothing could take off is named: {said}"
    );
}

/// A container engine that is not there at all is a different answer from one that
/// ran and refused, and both stop the install — the engine's own words underneath
/// the install's account rather than in place of it.
#[tokio::test]
async fn an_engine_that_cannot_be_run_at_all_stops_the_install() {
    let runner = Arc::new(Recording::answering(Err(
        lemonfiber_ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));
    let ctx = proving("no-engine", runner, Fake::silent());

    assert_eq!(
        beneath(installing(&ctx, &source("no-engine", PROVING)).await),
        ("PLUGIN-9".to_owned(), "PROC-1".to_owned()),
        "the install's own account leads, and what the engine said is carried \
             underneath it rather than dropped"
    );
}

/// A proof naming a method the transport cannot send is never put, and says so.
/// Sending a different verb than the one written down would be proving something
/// nobody declared.
#[tokio::test]
async fn a_proof_naming_a_method_lemonfiber_cannot_send_is_never_put() {
    let patched = PROVING.replace("method = \"GET\"", "method = \"PATCH\"");
    let ctx = proving(
        "patched",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );

    let stated = verdicts(installing(&ctx, &source("patched", &patched)).await);
    assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unproven");
    assert!(why(stated.first().and_then(Option::as_ref)).contains("PATCH"));
}

/// A container the engine would not take off the machine is named as still
/// standing, because *some of it worked* is the sentence that sends somebody
/// looking by hand.
#[tokio::test]
async fn a_container_that_would_not_come_off_is_named_as_still_standing() {
    let runner = Keyed::answering(
        vec![("rm", Ok(engine_refused("no such container")))],
        Ok(spoke("")),
    );
    let ctx = proving("stuck", runner.clone(), answering(503));

    let left = report(installing(&ctx, &source("stuck", PROVING)).await)
        .and_then(|one| one.install)
        .and_then(|one| one.reversed)
        .map(|back| back.left)
        .unwrap_or_default();
    assert_eq!(left.len(), 1);
    assert!(left
        .first()
        .is_some_and(|one| one.because.contains("could not be taken off")));
    assert!(
        runner.ran("rm"),
        "and it was asked, which is what makes the refusal the engine's rather than \
             an account of a call nobody made"
    );
}

/// An install that found everything it would write already there journals nothing
/// of its own, so a reversal has no run of its stamp to put back. That is the true
/// answer rather than a second failure: those files belong to the run that wrote
/// them and are on the record under it.
#[tokio::test]
async fn an_install_that_wrote_nothing_has_nothing_of_its_own_to_put_back() {
    let ctx = proving(
        "leftovers",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(503),
    );
    let stack = stack_of(&ctx);
    assert!(std::fs::create_dir_all(stack.join("config/komga")).is_ok());
    assert!(std::fs::create_dir_all(stack.join("compose/plugins")).is_ok());
    assert!(std::fs::write(stack.join("compose/plugins/komga.yml"), "services: {}\n").is_ok());

    let put_back = report(installing(&ctx, &source("leftovers", PROVING)).await)
        .and_then(|one| one.install)
        .and_then(|one| one.reversed)
        .unwrap_or_default();
    assert!(put_back.reversed.is_empty());
    assert!(put_back.left.is_empty());
}

/// A rehearsal asks nothing, so it states the proofs with no verdict against them
/// and says nothing about what answered — which is a different fact from a proof
/// that was asked and established nothing.
#[tokio::test]
async fn a_rehearsed_install_states_its_proofs_and_asks_none_of_them() {
    let ctx = {
        let mut ctx = proving(
            "unasked",
            Arc::new(Recording::answering(Ok(spoke("")))),
            Fake::silent(),
        );
        ctx.dry_run = true;
        ctx
    };
    let shown =
        report(installing(&ctx, &source("unasked", PROVING)).await).and_then(|one| one.install);

    assert_eq!(shown.as_ref().and_then(|one| one.against), None);
    let stated: Vec<Option<Verdict>> = shown
        .map(|one| one.proofs.into_iter().map(|proof| proof.came_to).collect())
        .unwrap_or_default();
    assert_eq!(stated.len(), 1, "the proof it would run is stated");
    assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unasked");
}

/// A stack directory is where a plugin's container has to go, so a machine
/// without one is refused by name rather than installed half-way.
#[tokio::test]
async fn a_machine_with_no_stack_directory_refuses_the_install_naming_it() {
    let env_file = env_at("no-stack", &a_password());
    let ctx = a_context()
        .settings(crate::config::Settings {
            env_file: Some(env_file),
            stack_dir: None,
            ..crate::config::Settings::default()
        })
        .build();
    assert_eq!(
        refusal(installing(&ctx, &source("no-stack", MANIFEST)).await),
        "PLUGIN-6"
    );
}

/// And a rehearsal on that same machine is refused in the same words, because a
/// rehearsal is the account the install then follows. One that answered here
/// would be describing an operation this machine cannot carry out, and the
/// operator would find that out on the run they thought they had checked. What
/// answers with no machine at all is `plugin claims`, which is a different
/// question.
#[tokio::test]
async fn a_rehearsal_is_refused_wherever_the_install_would_be() {
    let env_file = env_at("no-stack-rehearsed", &a_password());
    let mut ctx = a_context()
        .settings(crate::config::Settings {
            env_file: Some(env_file),
            stack_dir: None,
            ..crate::config::Settings::default()
        })
        .build();
    ctx.dry_run = true;
    assert_eq!(
        refusal(installing(&ctx, &source("no-stack-rehearsed", MANIFEST)).await),
        "PLUGIN-6"
    );
}

/// A proof is put only where its path is a route on the service it asks.
///
/// The reader refuses any other path, so an install never reaches here with one;
/// this is the join itself, asked directly. The transport would answer the path
/// with what the proof expects, so a proof that had been put would have held.
#[tokio::test]
async fn a_proof_whose_path_is_not_a_route_on_its_service_is_never_put() {
    let ctx = proving(
        "unrouted",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    for path in ["@elsewhere:9000/api/v1/libraries", "api/v1/libraries"] {
        let text = PROVING.replace("/api/v1/libraries", path);
        let read = lemonfiber_plugin::Manifest::from_toml(&text);
        assert!(read.is_ok(), "the fixture reads: {read:?}");
        let Some(proof) = read
            .ok()
            .and_then(|manifest| manifest.proofs.into_iter().next())
        else {
            continue;
        };
        let now = ctx.seams.clock.now();
        let verdict =
            super::super::proving::answering(&ctx, &proof, "http://komga:25600", now).await;
        assert_eq!(came_to(Some(&verdict)), "unproven", "{path}");
        assert!(
            why(Some(&verdict)).contains("not a route"),
            "{path}: {verdict:?}"
        );
    }
}

use super::*;

/// A sign-in by `id`.
fn signed(id: &str) -> Signed {
    Signed {
        id: id.to_owned(),
        token: "device".to_owned(),
    }
}

/// A household knowing one account recognises it, keeps its sign-in standing, and lists it,
/// until the account is withdrawn or its password changes.
#[tokio::test]
async fn a_known_account_stands_until_it_is_withdrawn_or_moves_on() {
    let household = Recognising::knowing("a7f3");
    assert!(household
        .whoever("ana", "pw", "device")
        .await
        .is_ok_and(|who| who.is_some_and(|who| who.id == "a7f3")));
    assert!(household
        .standing(&signed("a7f3"))
        .await
        .is_ok_and(|stands| stands));
    assert!(household
        .household()
        .await
        .is_ok_and(|members| members.len() == 1));

    household.change_password();
    assert!(household
        .standing(&signed("a7f3"))
        .await
        .is_ok_and(|stands| !stands));
    household.withdraw();
    assert!(household
        .household()
        .await
        .is_ok_and(|members| members.is_empty()));
}

/// A household knowing nobody recognises nobody, and one gone dark answers nothing.
#[tokio::test]
async fn nobody_is_recognised_and_a_dark_household_answers_nothing() {
    let nobody = Recognising::knowing_nobody();
    assert!(nobody
        .whoever("ana", "pw", "device")
        .await
        .is_ok_and(|who| who.is_none()));

    let dark = Recognising::unreachable();
    assert!(dark.whoever("ana", "pw", "device").await.is_err());
    assert!(dark.standing(&signed("a7f3")).await.is_err());
    assert!(dark.household().await.is_err());

    let gone = Recognising::knowing("a7f3");
    gone.go_dark();
    assert!(gone.household().await.is_err());
}

/// Every question but who somebody is is refused.
#[tokio::test]
async fn every_other_question_is_refused() {
    let household = Recognising::knowing("a7f3");
    let allowed = Allowed::default();

    assert!(household.invite("ana").await.is_err());
    assert!(household.claim("ana", "pw", "device").await.is_err());
    assert!(household.unclaim("a7f3").await.is_err());
    assert!(Household::withdraw(&*household, "a7f3").await.is_err());
    assert!(household.when_invited("2026-01-01").await.is_err());
    assert!(household.libraries().await.is_err());
    assert!(household.ratings().await.is_err());
    assert!(household.country().await.is_err());
    assert!(household.allow("a7f3", &allowed).await.is_err());
    assert!(household.claimable("a7f3", &allowed).await.is_err());
    assert!(household.suspend("a7f3").await.is_err());
    assert!(household.sessions("a7f3").await.is_err());
    assert!(household.signs_devices_in().await.is_err());
}

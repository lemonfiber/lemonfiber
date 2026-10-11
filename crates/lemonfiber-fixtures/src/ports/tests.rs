/// A resolver stand-in answers every name alike and remembers each one it was asked.
#[tokio::test]
async fn a_resolver_stand_in_answers_alike_and_remembers_what_it_was_asked() {
    use lemonfiber_ports::resolve::Resolver as _;

    let anywhere = super::Resolving::anywhere();
    assert_eq!(
        anywhere.addresses("example.org", 443).await,
        Ok(vec![super::ANYWHERE])
    );
    assert_eq!(anywhere.asked(), [("example.org".to_owned(), 443)]);

    let failing = super::Resolving::failing("no such host");
    assert_eq!(
        failing.addresses("example.org", 443).await,
        Err("no such host".to_owned())
    );
}

/// A rebinding stand-in answers one way the first time and another every time after.
#[tokio::test]
async fn a_rebinding_stand_in_answers_differently_after_the_first_time() {
    use lemonfiber_ports::resolve::Resolver as _;
    use std::net::IpAddr;

    let first = IpAddr::from([192, 88, 99, 1]);
    let then = IpAddr::from([10, 0, 0, 5]);
    let rebinding = super::Resolving::rebinding(&[first], &[then]);
    assert_eq!(
        rebinding.addresses("example.org", 443).await,
        Ok(vec![first])
    );
    assert_eq!(
        rebinding.addresses("example.org", 443).await,
        Ok(vec![then])
    );
    assert_eq!(
        rebinding.addresses("example.org", 443).await,
        Ok(vec![then])
    );
}

//! What a registry serves by digest, held to the digest it was asked by.

use lemonfiber_ports::http::{Http, Method, Request};

/// The header a registry names where it hands a read on to.
const LOCATION: &str = "Location";

/// What the registry at `host` serves at `url`, named by `digest`: followed once where the
/// registry hands it on to an https address, which is asked with no token, no more than
/// `most` bytes of it held where it is given, and refused unless its body is what `digest`
/// names.
pub(super) async fn by_digest(
    reaching: &dyn Http,
    (host, url, accept): (&str, &str, &str),
    (digest, most): (&str, Option<usize>),
    held: &mut Option<String>,
) -> Result<String, String> {
    let mut answer = super::authorised_within(reaching, (host, url, accept), most, held).await?;
    if (300..400).contains(&answer.status) {
        let to = answer
            .header(LOCATION)
            .filter(|to| to.starts_with("https://"))
            .ok_or_else(|| {
                format!(
                    "{url} answered {} and handed it on to no https address",
                    answer.status
                )
            })?
            .to_owned();
        let handed = Request {
            method: Method::Get,
            url: to,
            headers: vec![("Accept".to_owned(), accept.to_owned())],
            body: None,
            pinned: None,
            deadline: None,
        };
        answer = super::put(reaching, &handed, most).await?;
    }
    if !answer.is_success() {
        return Err(format!("{url} answered {}", answer.status));
    }
    let served = format!("sha256:{}", lemonfiber_ports::digest::sha256(&answer.body));
    if served != digest {
        return Err(format!("{url} served {served}, which is not {digest}"));
    }
    Ok(answer.body)
}

/// What the registry at `host` serves at `url` by `digest`, read as JSON.
pub(super) async fn json_by_digest(
    reaching: &dyn Http,
    (host, url, accept): (&str, &str, &str),
    (digest, most): (&str, Option<usize>),
    held: &mut Option<String>,
) -> Result<serde_json::Value, String> {
    let body = by_digest(reaching, (host, url, accept), (digest, most), held).await?;
    serde_json::from_str(&body).map_err(|why| format!("{url} answered no JSON: {why}"))
}

#[cfg(test)]
pub(super) mod tests;

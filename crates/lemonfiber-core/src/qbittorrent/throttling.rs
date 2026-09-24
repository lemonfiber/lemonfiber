//! Holding qBittorrent to a share of the line, on the household's own hours.
//!
//! qBittorrent keeps two sets of rate limits and a scheduler that switches between
//! them, which is exactly the shape a household needs and is why the schedule is
//! written into the client rather than acted on from out here. The mapping is
//! deliberate and is the whole of this file's cleverness:
//!
//! | The household's day | qBittorrent's |
//! |---|---|
//! | Active hours, limited | the *alternative* limits, inside the scheduled window |
//! | Quiet hours, unlimited | the ordinary limits, outside it |
//!
//! It falls that way round because qBittorrent's scheduler puts the alternative
//! limits in force *inside* its window, and the window a household can describe is
//! the one it is awake for.
//!
//! Two things follow that nothing else here could give. The client applies the
//! change to transfers already running, because a global rate limit is not a
//! property of a transfer; and the boundary is crossed on the container's own
//! clock, which the stack sets from `TZ` — so the household's zone, and its
//! daylight-saving transitions, are the client's to observe rather than something
//! this product has to model.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{
    Failure, Hours, Metering, Moved, Rates, Throttled, Throttling, Wanted,
};

use super::Qbittorrent;

/// qBittorrent's `scheduler_days` for "every day".
const EVERY_DAY: u8 = 0;

/// The figure qBittorrent writes for a limit that holds nothing back.
const NO_LIMIT: u64 = 0;

/// The preference fields the limits and the schedule are read from. The many
/// others qBittorrent sends are ignored.
#[derive(Deserialize)]
struct Limits {
    #[serde(default)]
    dl_limit: i64,
    #[serde(default)]
    up_limit: i64,
    #[serde(default)]
    alt_dl_limit: i64,
    #[serde(default)]
    alt_up_limit: i64,
    #[serde(default)]
    scheduler_enabled: bool,
}

/// The transfer figures a rate and a running total are read off. The many others
/// qBittorrent sends are ignored.
///
/// The two `_data` counts are what has moved since the client last started, which
/// is why anything built on them says so: a client restarted this morning has
/// forgotten the three weeks before it.
#[derive(Deserialize)]
struct Moving {
    #[serde(default)]
    dl_info_speed: u64,
    #[serde(default)]
    up_info_speed: u64,
    #[serde(default)]
    dl_info_data: u64,
    #[serde(default)]
    up_info_data: u64,
}

impl Qbittorrent {
    /// Whether the alternative limits — the household's active hours — are the
    /// ones in force this moment.
    ///
    /// Read from the client rather than worked out here, which is what makes the
    /// reported period a measurement. Nothing in this product knows the
    /// household's local time of day, and the client is the thing that does.
    async fn on_the_alternative(&self) -> Result<bool, Failure> {
        let response = self.get("/transfer/speedLimitsMode").await?;
        self.endpoint.expect_success(&response)?;
        Ok(response.body.trim() == "1")
    }

    /// The limits and the schedule as the client holds them.
    async fn limits(&self) -> Result<Limits, Failure> {
        let response = self.get("/app/preferences").await?;
        self.endpoint
            .decode(&response, "the rate limits could not be read")
    }

    /// What the client is moving and what it has moved, in one read.
    ///
    /// One call for both because qBittorrent answers both from one endpoint, and
    /// two reads of it would be two moments in a report describing one.
    async fn transfer(&self) -> Result<Moving, Failure> {
        self.signed_in().await?;
        let response = self.get("/transfer/info").await?;
        self.endpoint
            .decode(&response, "the transfer figures could not be read")
    }
}

#[async_trait]
impl Throttling for Qbittorrent {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        throttled(self).await
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        restrain(self, wanted).await
    }

    async fn moving(&self) -> Result<Rates, Failure> {
        let moving = self.transfer().await?;
        Ok(Rates {
            down: Some(moving.dl_info_speed),
            up: Some(moving.up_info_speed),
        })
    }
}

#[async_trait]
impl Metering for Qbittorrent {
    async fn moved(&self, _month: &str) -> Result<Moved, Failure> {
        // The month is not this client's to answer for. It keeps a running total
        // since it last started and nothing by calendar day, so the honest answer
        // is what it has and the flag that says which period that is.
        let moved = self.transfer().await?;
        Ok(Moved {
            down: moved.dl_info_data,
            up: moved.up_info_data,
            since_start: true,
        })
    }
}

/// One of qBittorrent's limit figures as a limit, where it is one.
///
/// The client writes a zero for "nothing holds this back", and a negative for a
/// preference it has no value for. Both are the absence of a limit, and neither is
/// a limit of nothing — which is what a client asked to move zero bytes a second
/// would be.
fn holding(figure: i64) -> Option<u64> {
    u64::try_from(figure).ok().filter(|held| *held > NO_LIMIT)
}

/// A limit as the figure qBittorrent writes for it, with no limit as its zero.
///
/// Clamped to what the client's own field can hold, so a limit larger than the
/// client can express arrives as the largest it can rather than wrapping into a
/// small one — which would be the one failure worse than not applying it.
fn figure(limit: Option<u64>) -> i64 {
    limit.map_or(0, |bytes| i64::try_from(bytes).unwrap_or(i64::MAX))
}

async fn throttled(qbittorrent: &Qbittorrent) -> Result<Throttled, Failure> {
    qbittorrent.signed_in().await?;
    let held = qbittorrent.limits().await?;
    let alternative = qbittorrent.on_the_alternative().await?;
    let (down, up) = if alternative {
        (held.alt_dl_limit, held.alt_up_limit)
    } else {
        (held.dl_limit, held.up_limit)
    };
    Ok(Throttled {
        rates: Rates {
            down: holding(down),
            up: holding(up),
        },
        uploads: true,
        hours: held.scheduler_enabled.then_some(if alternative {
            Hours::Active
        } else {
            Hours::Quiet
        }),
    })
}

async fn restrain(qbittorrent: &Qbittorrent, wanted: &Wanted) -> Result<Throttled, Failure> {
    qbittorrent.signed_in().await?;

    // Without a window there is nothing to switch between, so both sides get
    // the constrained rates and the scheduler is switched off — the household
    // is protected around the clock rather than at no point in it.
    let (ordinary, alternative) = match wanted.window {
        Some(_) => (wanted.quiet, wanted.active),
        None => (wanted.active, wanted.active),
    };
    // Built as a map rather than indexed into. Indexing a `Value` by name
    // panics where the value is not an object, so the shape would be trusted at
    // every one of these lines rather than stated once here.
    let mut asked = serde_json::Map::from_iter([
        ("dl_limit".to_owned(), figure(ordinary.down).into()),
        ("up_limit".to_owned(), figure(ordinary.up).into()),
        ("alt_dl_limit".to_owned(), figure(alternative.down).into()),
        ("alt_up_limit".to_owned(), figure(alternative.up).into()),
        (
            "scheduler_enabled".to_owned(),
            wanted.window.is_some().into(),
        ),
    ]);
    if let Some(window) = wanted.window {
        asked.extend([
            ("schedule_from_hour".to_owned(), window.from_hour.into()),
            ("schedule_from_min".to_owned(), window.from_minute.into()),
            ("schedule_to_hour".to_owned(), window.to_hour.into()),
            ("schedule_to_min".to_owned(), window.to_minute.into()),
            ("scheduler_days".to_owned(), EVERY_DAY.into()),
        ]);
    }
    let asked = serde_json::Value::Object(asked).to_string();

    let request = qbittorrent.post("/app/setPreferences", &[("json", &asked)]);
    let response = qbittorrent.endpoint.send(&request).await?;
    qbittorrent.endpoint.expect_success(&response)?;

    // Read back rather than echoed. A client that accepted the write and did
    // not apply it looks exactly like one that did, from here.
    qbittorrent.throttled().await
}

#[cfg(test)]
mod tests;

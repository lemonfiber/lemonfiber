//! The code a part claimed against an account outside the house is claimed with.
//!
//! The outside account issues it and it holds for `lapses_s` seconds. lemonfiber cannot
//! mint it and does not keep it: the operator enters it, unshown, only when a claim of
//! the part finds it open, it reaches the adapter through `upstream.json` alone, and it
//! is removed from the secret store and from `upstream.json` once the claim answers or
//! it lapses, whichever is first.

use std::path::PathBuf;

use crate::app::Ctx;
use crate::plugin::{Installed, Placed};
use crate::wiring::Filler;

/// The codes one speaker's part is claimed with, and where they are written.
pub(super) struct Coded<'i> {
    /// The plugin the speaker is one of.
    installed: &'i Installed,
    /// The speaker, whose `upstream.json` hands the codes over.
    placed: &'i Placed,
    /// Each code's setting, with the seconds it holds and what it is for.
    codes: Vec<Code<'i>>,
}

/// One code: its setting, the seconds it holds once entered, and what it is for.
struct Code<'i> {
    setting: String,
    lapses_s: u32,
    why: &'i str,
}

impl<'i> Coded<'i> {
    /// The codes `speaker`'s adapter hands its upstream, among `installed`; nothing where it
    /// hands none.
    pub(super) fn of(installed: &'i [Installed], speaker: &Filler) -> Option<Self> {
        installed.iter().find_map(|one| {
            let placed = one
                .services
                .iter()
                .find(|placed| placed.service == speaker.id)?;
            let codes: Vec<Code<'i>> = one
                .declared
                .secrets
                .iter()
                .filter(|secret| placed.upstream.values().any(|id| *id == secret.id))
                .filter_map(|secret| {
                    secret.lapses_s.map(|lapses_s| Code {
                        setting: crate::settings::named(&one.plugin, &secret.id),
                        lapses_s,
                        why: &secret.why,
                    })
                })
                .collect();
            (!codes.is_empty()).then_some(Self {
                installed: one,
                placed,
                codes,
            })
        })
    }

    /// Each code held past the seconds it holds removed, from the store and from
    /// `upstream.json`.
    ///
    /// # Errors
    ///
    /// Why the store or `upstream.json` could not be written.
    pub(super) fn lapse(&self, ctx: &Ctx) -> Result<(), String> {
        let now = ctx.seconds();
        let secrets = held(ctx);
        let lapsed: Vec<String> = self
            .codes
            .iter()
            .filter(|code| {
                secrets
                    .entered_at(&code.setting)
                    .is_some_and(|at| now.saturating_sub(at) >= u64::from(code.lapses_s))
            })
            .map(|code| code.setting.clone())
            .collect();
        if lapsed.is_empty() {
            return Ok(());
        }
        self.dropped(ctx, &lapsed)
    }

    /// Every code held, asking the operator, unshown, for each one that is not, and
    /// writing them into `upstream.json`: whether all of them are held.
    ///
    /// # Errors
    ///
    /// Why a code given could not be kept or written.
    pub(super) fn entered(&self, ctx: &Ctx) -> Result<bool, String> {
        let secrets = held(ctx);
        let wanted: Vec<&Code<'_>> = self
            .codes
            .iter()
            .filter(|code| secrets.value(&code.setting).is_none())
            .collect();
        if wanted.is_empty() {
            return Ok(true);
        }
        if !ctx.asking.present() {
            return Ok(false);
        }
        let store = store(ctx)?;
        let at = ctx.seconds();
        for code in wanted {
            let typed = ctx
                .asking
                .secret(&format!("{} ({})", code.why, code.setting));
            if typed.is_empty() {
                return Ok(false);
            }
            crate::settings::lapsing(&store, &code.setting, &typed, at)
                .map_err(|failure| failure.to_string())?;
        }
        self.written(ctx).map(|()| true)
    }

    /// Every code removed, from the store and from `upstream.json`, once the claim it was
    /// entered for answered.
    ///
    /// # Errors
    ///
    /// Why the store or `upstream.json` could not be written.
    pub(super) fn removed(&self, ctx: &Ctx) -> Result<(), String> {
        let settings: Vec<String> = self.codes.iter().map(|code| code.setting.clone()).collect();
        self.dropped(ctx, &settings)
    }

    /// Why a part `name` is left open where no code was given, naming each code.
    pub(super) fn unentered(&self, name: &str) -> String {
        let settings: Vec<&str> = self
            .codes
            .iter()
            .map(|code| code.setting.as_str())
            .collect();
        format!(
            "nobody holds {name}, and claiming it takes {}, the code the account it is claimed \
             with shows, which was not entered",
            settings.join(", ")
        )
    }

    /// `settings` removed from the store, and `upstream.json` written without them.
    fn dropped(&self, ctx: &Ctx, settings: &[String]) -> Result<(), String> {
        crate::settings::dropped(&store(ctx)?, settings).map_err(|failure| failure.to_string())?;
        self.written(ctx)
    }

    /// The speaker's `upstream.json` written with what the store holds now.
    fn written(&self, ctx: &Ctx) -> Result<(), String> {
        let stack = crate::app::project::directory(ctx).ok_or_else(nowhere)?;
        crate::app::credentials::upstream_anew(ctx, self.installed, &stack, self.placed)
    }
}

/// The secret store as it is now; empty where there is none.
fn held(ctx: &Ctx) -> crate::settings::Secrets {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .unwrap_or_default()
}

/// Where the secret store is.
fn store(ctx: &Ctx) -> Result<PathBuf, String> {
    crate::settings::secrets_at(ctx).ok_or_else(nowhere)
}

/// Said where there is no stack to write in.
fn nowhere() -> String {
    crate::config::store::Failure::Nowhere.to_string()
}

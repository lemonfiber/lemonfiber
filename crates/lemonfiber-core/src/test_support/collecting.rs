//! The curators of music and books speaking `music.curate` and `books.curate`: Lidarr's and
//! Bindery's plugins as the bundle pins them, each adapter recorded as speaking its
//! medium's contract and filling it.

use lemonfiber_contract::capabilities::{books, music};

use super::adapted::{Adapted, PORT};
use super::curators::MUSIC;
use crate::app::Ctx;
use crate::collected::Medium;
use crate::plugin::Register;

/// Bindery's plugin and the adapter in front of it.
pub(crate) const BOOKS: Adapted = Adapted {
    plugin: super::books::PLUGIN,
    adapter: super::books::ADAPTER,
    port: PORT,
};

/// The curator of `medium`, and the contract it speaks.
const fn of(medium: Medium) -> (Adapted, &'static str, u32) {
    match medium {
        Medium::Music => (MUSIC, music::curate::CAPABILITY, music::curate::MAJOR),
        Medium::Books => (BOOKS, books::curate::CAPABILITY, books::curate::MAJOR),
    }
}

/// Where `operation` of the curator of music's contract is asked.
pub(crate) fn at(operation: &str) -> &'static str {
    at_of(Medium::Music, operation)
}

/// Where `operation` of the curator of `medium`'s contract is asked.
pub(crate) fn at_of(medium: Medium, operation: &str) -> &'static str {
    let (curator, capability, major) = of(medium);
    curator.at(capability, major, operation)
}

/// `ctx` with the curator of music moved in, speaking its contract.
pub(crate) fn speaking(ctx: &Ctx) {
    speaking_of(ctx, Medium::Music);
}

/// `ctx` with the curator of `medium` moved in, its adapter keyed and recorded as speaking
/// its medium's contract and filling it. Nothing where `ctx` names no env file.
pub(crate) fn speaking_of(ctx: &Ctx, medium: Medium) {
    let (curator, capability, major) = of(medium);
    curator.moved_in(ctx);
    let Some(kept) = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::PLUGINS))
    else {
        return;
    };
    let mut register = std::fs::read_to_string(&kept)
        .ok()
        .and_then(|text| Register::parse(&text).ok())
        .unwrap_or_else(Register::empty);
    let Some(mut installed) = register.holds(curator.plugin).cloned() else {
        return;
    };
    for placed in &mut installed.services {
        if placed.service == curator.adapter {
            placed.speaks.push(format!("{capability}@{major}"));
            placed.provides.push(capability.to_owned());
        }
    }
    register.forget(curator.plugin);
    let _ = register.record(installed);
    let _ = std::fs::write(&kept, register.to_json().unwrap_or_default());
}

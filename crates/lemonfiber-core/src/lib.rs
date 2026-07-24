//! Core logic for lemonfiber: stack lifecycle, Docker access, diagnostics,
//! seeding, config, platform detection.
//!
//! This crate has NO user-interface dependency of any kind — no terminal, no
//! HTTP server. That boundary makes "surfaces are renderings, never
//! capabilities" structural rather than aspirational. See spec
//! `20-architecture/component-model.md` (`ARCH-R11`).
//!
//! Skeleton — modules are stubs.

/// The name of the product, as it appears to users.
pub const PRODUCT: &str = "lemonfiber";

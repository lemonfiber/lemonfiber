//! What one run was asked about what the operator is told about.
//!
//! Two asks under one word. The reading is the whole of what a surface other than the
//! terminal offers, because taking a preset makes the product quieter — a fault it
//! stops mentioning goes unmentioned until somebody next looks — and that is a choice
//! to make in front of the person making it.
//!
//! Taking a preset leaves the individual exceptions in place. A broader answer is not a
//! reason to discard the specific ones already given, and an operator who has switched
//! one event off has said something more particular than the preset says.

use crate::alert::Appetite;

/// What a notifications command asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertAction {
    /// Show the preset in force, what it means, and anything set apart from it.
    Show,
    /// Take a preset, leaving the individual exceptions in place.
    Set(Appetite),
}

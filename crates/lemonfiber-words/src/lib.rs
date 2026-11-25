//! How lemonfiber words what it shows, with no stack behind it: text made safe for a
//! terminal, byte counts, ratios, waits and age limits as a person reads them, how a
//! hand-edited setting reads, and the words it explains where it uses them. Every
//! surface and the core read the same wording from here.

pub mod age;
pub mod bytes;
pub mod glossary;
pub mod ratio;
pub mod setting;
pub mod text;
pub mod wait;

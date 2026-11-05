//! Whether a plugin's adapters answer the contracts they speak, judged from the
//! recordings the plugin ships and nothing else.
//!
//! Each case of each contract an adapter speaks has one recording beside the manifest,
//! at `conformance/<capability>@<major>/<case>.json`, taken against the upstream the
//! adapter fronts at the digest the manifest pins it by. A recorded answer is read by
//! the very client the core asks a running adapter with, so what passes here is what
//! the core would read and what fails is what it would refuse.
//!
//! One judge, read alike by `lemonfiber plugin conform`, by every install and update,
//! and by the build that compiles the bundle into the core. It reads no plugin manifest:
//! each reader hands it what each adapter speaks, and what the upstream it answers for is
//! pinned at and files. Nothing it depends on depends on the compiled bundle, so that
//! build can depend on it.

pub mod recording;

mod judging;

pub use judging::{
    judged, judged_now, owed, recording_of, Judged, Owed, Speaker, Verdict, ASKED_WITH, RECORDINGS,
};

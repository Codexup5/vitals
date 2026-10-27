//! What could be cleaned and the cleaning itself, under the rules of the brief, §3: a preview lists the files, the
//! run deletes exactly that list inside the allowed roots, links are never followed, every run is logged.

mod docker;
pub mod elevated;
mod execute;
mod guard;
pub mod plan;
mod process;
mod snapshot;
mod sources;
#[cfg(windows)]
mod windows;

pub use vitals_ipc::{Candidate, CandidateKind, Group};

pub use sources::{Places, find_candidates};

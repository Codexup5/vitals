//! What could be cleaned and how much it holds — the preview of the brief, §3. Nothing here deletes anything:
//! the cleaning engine comes with stage 3b, behind the tests of every invariant of §3.

mod docker;
mod sources;

pub use vitals_ipc::{Candidate, CandidateKind, Group};

pub use sources::{Places, find_candidates};

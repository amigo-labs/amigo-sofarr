//! Parity tooling: collects oracle fixtures from running Radarr/Sonarr instances
//! (black-box, via their HTTP API only) and diffs sofarr's behaviour against them.
//!
//! Implementation: PLAN.md Phase 1 (`collect`), Phase 2 (`diff releases`).

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("sofarr-parity ist noch nicht implementiert – siehe PLAN.md, Phase 1.");
    ExitCode::from(2)
}

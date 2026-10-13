//! `csp`, the sanctioned short name for `conspectus` (ADR 0118). The same
//! program under a shorter name; help and suggested commands follow the
//! name it was run as.

fn main() -> anyhow::Result<()> {
    conspectus::cli::run()
}

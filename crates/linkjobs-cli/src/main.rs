//! `cargo run -p linkjobs-cli -- --root target/linkjobs` — Ctrl-C stops gracefully.

fn main() -> rivium_host::Code {
    rivium_host::process::run::<linkjobs_core::LinkJobs>()
}

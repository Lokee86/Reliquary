//! Run with `cargo run --example f03_identity_observation --features history-oracle-fixtures --locked --offline -- --source-root <verified-copies> --output <fresh-protected-output>`.
fn main() {
    std::process::exit(reliquary_memory::f03_identity_observation::cli_main());
}

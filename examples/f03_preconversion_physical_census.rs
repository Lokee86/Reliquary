//! One-shot physical-only scan over the V01 hash/length-pinned copies.
//! The command intentionally returns exit code 2 because semantic admission remains incomplete.
fn main() {
    std::process::exit(
        reliquary_memory::f03_source_witness::preconversion_physical_census::cli_main(),
    );
}

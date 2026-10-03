use reliquary_memory::Cva;
use std::env;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if !(2..=3).contains(&args.len()) {
        return Err("usage: archive_open_profile <archive.cva> [runs]".into());
    }
    let runs = args
        .get(2)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(25);

    let mut timings = Vec::with_capacity(runs);

    for _ in 0..runs {
        let start = Instant::now();
        let archive = Cva::open(&args[1])?;
        let elapsed = start.elapsed();

        timings.push(elapsed.as_nanos() as u64);

        std::hint::black_box(archive.stats());
        drop(archive);
    }

    timings.sort_unstable();
    println!(
        "open-profile runs={} median_us={} p90_us={} allocator_retained_bytes=unavailable allocator_peak_bytes=unavailable",
        runs,
        percentile(&timings, 50) / 1_000,
        percentile(&timings, 90) / 1_000,
    );
    Ok(())
}

fn percentile(values: &[u64], percent: usize) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) * percent) / 100;
    values[index]
}

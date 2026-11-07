use d02_current_reader::{parse_cli_args, probe_candidate_page};
use std::{
    env,
    fs::OpenOptions,
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let (kind, candidate, output, memory_cursor) = match parse_cli_args(env::args().skip(1)) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let report = probe_candidate_page(kind, &candidate, memory_cursor.as_deref());
    let mut bytes = match serde_json::to_vec_pretty(&report) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to render reader probe: {error}");
            return ExitCode::from(2);
        }
    };
    bytes.push(b'\n');
    let write_result = match output {
        Some(path) => OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|mut file| file.write_all(&bytes)),
        None => io::stdout().lock().write_all(&bytes),
    };
    if let Err(error) = write_result {
        eprintln!("failed to write reader probe: {error}");
        return ExitCode::from(2);
    }
    ExitCode::from(2)
}

use d02_current_reader::{
    observe_archive_cut, observe_memory_history, parse_cli_args, probe_candidate_page,
};
use std::{
    env,
    fs::OpenOptions,
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let args = match parse_cli_args(env::args().skip(1)) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let report = match (args.archive_cut, args.memory_history) {
        (Some(cut), _) => observe_archive_cut(args.kind, &args.candidate, cut),
        (_, Some((id, cut))) => observe_memory_history(args.kind, &args.candidate, id, cut),
        (None, None) => {
            probe_candidate_page(args.kind, &args.candidate, args.memory_cursor.as_deref())
        }
    };
    let mut bytes = match serde_json::to_vec_pretty(&report) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to render reader probe: {error}");
            return ExitCode::from(2);
        }
    };
    bytes.push(b'\n');
    let write_result = match args.output {
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

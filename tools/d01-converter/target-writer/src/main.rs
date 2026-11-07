use reliquary_owner_session::{RelSession, RelWriter};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
    process::ExitCode,
    time::SystemTime,
};

mod transfer;
use transfer::{Transfer, decode_hex};

const MAX_TRANSFER_BYTES: u64 = 4 * 1024 * 1024;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match arguments.as_slice() {
        [mode, source, expected, output] if mode == "--synthetic-fixture" => {
            write(Path::new(source), expected.to_str(), Path::new(output))
        }
        _ => Err((2, "expected --synthetic-fixture TRANSFER SHA256 NEW-REL")),
    };
    match result {
        Ok(receipt) => {
            println!("{receipt}");
            ExitCode::SUCCESS
        }
        Err((code, reason)) => {
            eprintln!("{}", json!({ "status": "FAIL", "reason": reason }));
            ExitCode::from(code)
        }
    }
}

type Failure = (u8, &'static str);

fn write(source: &Path, expected: Option<&str>, output: &Path) -> Result<Value, Failure> {
    let expected: [u8; 32] = expected
        .and_then(|value| decode_hex(value).ok())
        .ok_or((3, "a canonical source SHA-256 is required"))?;
    let (bytes, original) = read_source(source)?;
    let hash: [u8; 32] = Sha256::digest(&bytes).into();
    if hash != expected {
        return Err((3, "transfer hash does not match"));
    }
    let transfer: Transfer =
        serde_json::from_slice(&bytes).map_err(|_| (4, "invalid synthetic Archive transfer"))?;
    let owner = transfer.owner().map_err(|reason| (4, reason))?;
    let scope = transfer.scope();
    let generation = transfer.generation().map_err(|reason| (4, reason))?;
    let name = output
        .file_name()
        .ok_or((2, "candidate file name is required"))?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|_| (7, "candidate parent is unavailable"))?;
    let output = parent.join(name);
    match output.symlink_metadata() {
        Ok(_) => return Err((5, "candidate already exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err((7, "could not inspect candidate destination")),
    }
    let stage = tempfile::Builder::new()
        .prefix(".d01-synthetic-")
        .tempdir_in(&parent)
        .map_err(|_| (7, "could not create candidate staging directory"))?;
    let staged_file = stage.path().join("owner.rel");
    let mut writer = RelWriter::create_current_path(&staged_file, scope, owner)
        .map_err(|_| (7, "could not initialize a current owner"))?;
    let cuts = transfer.replay(&mut writer).map_err(|reason| (4, reason))?;
    if cuts[0].1.history_generation != generation {
        return Err((4, "unsupported synthetic owner history generation"));
    }
    drop(writer);

    let reader = RelSession::open_path_current(&staged_file)
        .map_err(|_| (6, "candidate failed normal current-only reopen"))?;
    if reader.owner_id() != owner || reader.scope() != scope {
        return Err((6, "candidate owner identity changed on reopen"));
    }
    for (cut, proof) in &cuts {
        if reader.historical_proof(owner, cut).as_ref() != Ok(proof)
            || proof.history_generation != generation
        {
            return Err((6, "candidate historical proof changed on reopen"));
        }
    }
    if cuts.last().map(|(cut, _)| cut) != Some(&reader.current_historical_cut()) {
        return Err((6, "candidate head changed on reopen"));
    }
    drop(reader);

    let candidate_digest =
        file_hash(&staged_file).map_err(|_| (7, "could not hash reopened candidate"))?;
    let writer_digest = std::env::current_exe()
        .and_then(|path| file_hash(&path))
        .map_err(|_| (7, "could not hash current writer executable"))?;
    let (after, observed) = read_source(source)?;
    if original != observed || bytes != after {
        return Err((3, "transfer changed before candidate publication"));
    }
    fs::hard_link(&staged_file, &output).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            (5, "candidate already exists")
        } else {
            (7, "atomic no-replace candidate publication failed")
        }
    })?;
    let history = cuts
        .iter()
        .map(|(cut, proof)| {
            json!({
                "archive_version": cut.archive_version.to_string(),
                "retained_records": proof.retained_records.to_string(),
                "proof_sha256": hex::encode(proof.digest)
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_id": "reliquary-d01-synthetic-archive-receipt-v1",
        "status": "INCOMPLETE",
        "scope": "synthetic_archive_only",
        "authentic_source_admission": false,
        "current_only_owner_reopen": true,
        "owner": hex::encode(owner),
        "history_generation": hex::encode(generation),
        "source_sha256_before": hex::encode(expected),
        "source_sha256_after": hex::encode(expected),
        "candidate_sha256": candidate_digest,
        "writer_sha256": writer_digest,
        "archive_history": history
    }))
}

fn read_source(path: &Path) -> Result<(Vec<u8>, (u64, SystemTime)), Failure> {
    let mut file = File::open(path).map_err(|_| (7, "transfer is unavailable"))?;
    let stamp = |file: &File| -> io::Result<_> {
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::other("transfer is not a regular file"));
        }
        Ok((metadata.len(), metadata.modified()?))
    };
    let before = stamp(&file).map_err(|_| (7, "could not inspect transfer"))?;
    if before.0 > MAX_TRANSFER_BYTES {
        return Err((4, "synthetic transfer exceeds size bound"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_TRANSFER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| (7, "could not read transfer"))?;
    let after = stamp(&file).map_err(|_| (7, "could not inspect transfer"))?;
    if before != after || after.0 != bytes.len() as u64 {
        return Err((3, "transfer changed during read"));
    }
    Ok((bytes, after))
}

fn file_hash(path: &Path) -> io::Result<String> {
    let mut digest = Sha256::new();
    io::copy(&mut File::open(path)?, &mut digest)?;
    Ok(hex::encode(digest.finalize()))
}

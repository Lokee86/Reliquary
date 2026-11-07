mod frames;
use crate::{Failure, file_hash, transfer::decode_hex};
use frames::{Event, append, invalid, number};
use reliquary_container::{Container, ContainerIdentity, FileKind, ReliquaryScope};
use reliquary_owner_session::{PhySession, RelSession};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read},
    path::Path,
};

const SCHEMA: &str = "reliquary-d01-authentic-native-frames-v1";
const MAX_TRANSFER: u64 = 512 * 1024 * 1024;
const MAX_LINE: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    schema_id: String,
    source_sha256: String,
    source_frames: String,
    file_kind: String,
    owner_uuid: String,
    scope: String,
    global_head: String,
}

fn line(
    reader: &mut BufReader<File>,
    bytes: &mut Vec<u8>,
    digest: &mut Sha256,
) -> Result<bool, Failure> {
    bytes.clear();
    let read = reader
        .take(MAX_LINE + 1)
        .read_until(b'\n', bytes)
        .map_err(|_| (7, "could not read native transfer"))?;
    if read == 0 {
        return Ok(false);
    }
    if read as u64 > MAX_LINE || bytes.last() != Some(&b'\n') {
        return Err(invalid("invalid native transfer line bounds"));
    }
    digest.update(&*bytes);
    Ok(true)
}

fn destination(path: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), Failure> {
    let name = path
        .file_name()
        .ok_or((2, "candidate file name is required"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|_| (7, "candidate parent is unavailable"))?;
    let path = parent.join(name);
    match path.symlink_metadata() {
        Ok(_) => Err((5, "candidate already exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((path, parent)),
        Err(_) => Err((7, "could not inspect candidate destination")),
    }
}

pub fn convert(transfer: &Path, expected: Option<&str>, output: &Path) -> Result<Value, Failure> {
    let expected: [u8; 32] = expected
        .and_then(|value| decode_hex(value).ok())
        .ok_or((3, "a canonical transfer SHA-256 is required"))?;
    let file = File::open(transfer).map_err(|_| (7, "native transfer is unavailable"))?;
    let before = file
        .metadata()
        .map_err(|_| (7, "could not inspect native transfer"))?;
    if !before.is_file() || before.len() > MAX_TRANSFER {
        return Err(invalid("native transfer is not a bounded regular file"));
    }
    let mut reader = BufReader::new(file);
    let mut bytes = Vec::new();
    let mut digest = Sha256::new();
    if !line(&mut reader, &mut bytes, &mut digest)? {
        return Err(invalid("missing native transfer header"));
    }
    let header: Header =
        serde_json::from_slice(&bytes).map_err(|_| invalid("invalid native transfer header"))?;
    if header.schema_id != SCHEMA {
        return Err(invalid("unknown native transfer schema"));
    }
    let source: [u8; 32] = decode_hex(&header.source_sha256).map_err(invalid)?;
    let owner = decode_hex(&header.owner_uuid).map_err(invalid)?;
    let expected_head: u64 = number(&header.global_head)?;
    let expected_frames: u64 = number(&header.source_frames)?;
    let scope = match header.scope.as_str() {
        "unspecified" => ReliquaryScope::Unspecified,
        "project" => ReliquaryScope::Project,
        "organization" => ReliquaryScope::Organization,
        "connection" => ReliquaryScope::Connection,
        _ => return Err(invalid("unknown native owner scope")),
    };
    let identity = match header.file_kind.as_str() {
        "rel" => ContainerIdentity::reliquary(scope),
        "phy" if scope == ReliquaryScope::Unspecified => ContainerIdentity::phylactery(),
        _ => return Err(invalid("unsupported native owner identity")),
    };
    let (output, parent) = destination(output)?;
    let stage = tempfile::Builder::new()
        .prefix(".d01-authentic-")
        .tempdir_in(parent)
        .map_err(|_| (7, "could not create candidate staging directory"))?;
    let staged = stage.path().join("owner");
    let mut writer = Container::create(&staged, identity, owner)
        .map_err(|_| (7, "could not create native candidate"))?;
    let mut last = None;
    let mut emitted = 0u64;
    let mut memories = BTreeMap::new();
    while line(&mut reader, &mut bytes, &mut digest)? {
        let event: Event =
            serde_json::from_slice(&bytes).map_err(|_| invalid("invalid native transfer event"))?;
        let ordinal = event.ordinal()?;
        if ordinal >= expected_frames
            || match last {
                Some(n) => ordinal != n && ordinal != n + 1,
                None => ordinal != 0,
            }
        {
            return Err(invalid(
                "native transfer source ordinals are not exhaustive",
            ));
        }
        last = Some(ordinal);
        if append(event, &mut writer, &mut memories)? {
            emitted += 1;
        }
    }
    let observed: [u8; 32] = digest.finalize().into();
    let after = reader
        .get_ref()
        .metadata()
        .map_err(|_| (7, "could not reinspect native transfer"))?;
    if expected != observed
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err((3, "native transfer hash or custody changed"));
    }
    if last.map_or(0, |n| n + 1) != expected_frames || writer.latest_version() != expected_head {
        return Err(invalid("native source coverage or global head differs"));
    }
    writer
        .sync()
        .map_err(|_| (7, "could not sync native candidate"))?;
    drop(writer);
    let opened = match identity.file_kind {
        FileKind::Reliquary => RelSession::open_path_current(&staged).map(|r| r.owner_id()),
        FileKind::Phylactery => PhySession::open_path_current(&staged).map(|r| r.owner_id()),
    };
    let reopened = opened.map_err(|error| {
        eprintln!("native owner admission: {error}");
        (6, "candidate failed normal current-only owner reopen")
    })?;
    let held = Container::open_read_only_as(&staged, identity.file_kind)
        .map_err(|_| (6, "reopened candidate clock is unavailable"))?;
    if reopened != owner || held.identity() != identity || held.latest_version() != expected_head {
        return Err((
            6,
            "candidate identity or global head differs after normal reopen",
        ));
    }
    drop(held);
    let candidate = file_hash(&staged).map_err(|_| (7, "could not hash reopened candidate"))?;
    fs::hard_link(&staged, output).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            (5, "candidate already exists")
        } else {
            (7, "atomic no-replace candidate publication failed")
        }
    })?;
    Ok(json!({
        "status":"PASS", "scope":"create_only_current_owner_reopen",
        "independent_retained_data_comparison":false,
        "source_sha256":hex::encode(source), "source_frames":expected_frames.to_string(),
        "transfer_sha256":hex::encode(observed), "candidate_sha256":candidate,
        "global_head":expected_head.to_string(), "target_frames":emitted.to_string()
    }))
}

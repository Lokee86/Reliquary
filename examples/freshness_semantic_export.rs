//! Offline retained-vector inventory. Opens only a verified disposable REL copy.
//! No inference, admission, generation publication or source-file mutation.
use reliquary_memory::{Cva, ScalarType};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
fn digest(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn row(bytes: &[u8]) -> Result<Vec<f32>, Box<dyn Error>> {
    if !bytes.len().is_multiple_of(4) {
        return Err("invalid f32 row".into());
    }
    let v: Vec<_> = bytes
        .chunks_exact(4)
        .map(|x| f32::from_le_bytes(x.try_into().unwrap()))
        .collect();
    if v.iter().any(|x| !x.is_finite()) {
        return Err("nonfinite row".into());
    }
    Ok(v)
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: freshness_semantic_export <fixture> <sha256> <output-json>".into());
    }
    let source = Path::new(&args[0]);
    let expected = &args[1];
    if digest(source)? != *expected {
        return Err("source digest mismatch".into());
    }
    let output = Path::new(&args[2]);
    if source.canonicalize()? == output.canonicalize().unwrap_or(output.to_path_buf()) {
        return Err("output must not overwrite source".into());
    }
    let scratch =
        Scratch(std::env::temp_dir().join(format!("reliquary-semantic-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&scratch.0)?;
    let copy = scratch.0.join("fixture.rel");
    fs::copy(source, &copy)?;
    // fs::copy preserves Windows read-only flags; only the scratch copy is writable.
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(&copy)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&copy, permissions)?;
    }
    if digest(&copy)? != *expected {
        return Err("copy digest mismatch".into());
    }
    let result = (|| -> Result<serde_json::Value, Box<dyn Error>> {
        let mut cva = Cva::open(&copy)?;
        let mut ids = cva.memory_ids();
        ids.sort_by_key(|x| x.0);
        let profiles = cva.compatibility_profiles();
        let mut populations = Vec::new();
        for profile in profiles {
            let mut targets = Vec::new();
            let mut eligible = 0;
            let mut packed_cache = BTreeMap::new();
            for id in &ids {
                let memory = cva.memory(*id)?;
                if memory.archived
                    || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical")
                {
                    continue;
                }
                eligible += 1;
                let body = cva.memory_body_id(*id)?;
                let Some(loc) = cva.memory_vector_location(profile.id, body) else {
                    continue;
                };
                let set = cva.memory_vectors(loc.set_id)?;
                if let std::collections::btree_map::Entry::Vacant(e) =
                    packed_cache.entry(set.packed_vector_id.0)
                {
                    e.insert(cva.packed_vectors(set.packed_vector_id)?);
                }
                let packed = &packed_cache[&set.packed_vector_id.0];
                if packed.schema().scalar != ScalarType::F32 {
                    return Err("unsupported scalar".into());
                }
                let v = row(packed
                    .row(usize::try_from(loc.row)?)
                    .ok_or("missing target row")?)?;
                if v.len() != profile.dimensions as usize {
                    return Err("target dimension".into());
                }
                targets.push(json!({"id":hex(&id.0),"vector":v}));
            }
            let mut sources = Vec::new();
            let mut archive_rows = 0;
            if let Some(generation) = cva.current_vector_generation(profile.id) {
                let set = cva.archive_vectors(generation.archive_vector_id)?;
                let packed = cva.packed_vectors(set.packed_vector_id)?;
                if packed.schema().scalar != ScalarType::F32 {
                    return Err("unsupported archive scalar".into());
                }
                archive_rows = set.fragment_ids.len();
                let mut fragment_rows: Vec<_> = set.fragment_ids.iter().enumerate().collect();
                fragment_rows.sort_by_key(|(_, id)| id.0);
                // Deterministic broad sample: uniform ranks of FragmentId byte order.
                let count = fragment_rows.len().min(64);
                for index in 0..count {
                    let (ordinal, id) = fragment_rows[index * fragment_rows.len() / count];
                    let v = row(packed.row(ordinal).ok_or("missing source row")?)?;
                    sources.push(json!({"id":hex(&id.0),"vector":v}));
                }
            }
            let source_kind = if sources.is_empty() {
                let count = targets.len().min(16);
                for index in 0..count {
                    let target = &targets[index * targets.len() / count];
                    sources.push(json!({"id":target["id"],"memory_id":target["id"],"vector":target["vector"]}));
                }
                "memory-vector-surrogates-no-genuine-fragment-evidence"
            } else {
                "mixed-turn-fragment-neighborhood-only"
            };
            populations.push(json!({"profile":hex(&profile.id.0),"dimensions":profile.dimensions,"source_kind":source_kind,
                "lifecycle_eligible_memories":eligible,"target_vectors":targets,
                "archive_fragment_rows":archive_rows,"sources":sources}));
        }
        Ok(
            json!({"schema":2,"fixture_sha256":expected,"memory_count":ids.len(),
            "source_kind":"retained-mixed-turn-fragment-neighborhood-only","populations":populations}),
        )
    })();
    if digest(source)? != *expected {
        return Err("source changed during measurement".into());
    }
    fs::write(output, serde_json::to_vec(&result?)?)?;
    println!(
        "verified retained-vector export: {} -> {}",
        source.display(),
        output.display()
    );
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

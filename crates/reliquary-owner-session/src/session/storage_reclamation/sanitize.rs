//! Prepare a temporary semantically readable view of a source whose only
//! invalid tail consists of verified unpublished vector-generation frames.
use super::*;

pub(super) fn write_filtered_without_manifest(
    source: &Container,
    path: &Path,
    removed: &BTreeSet<u64>,
) -> Result<(), String> {
    let mut target =
        Container::create(path, source.identity(), source.owner_id()).map_err(|e| e.to_string())?;
    let mut relocated = BTreeMap::new();
    for_each_frame(source, |object, bytes| {
        if removed.contains(&object.offset) {
            return Ok(());
        }
        let receipt;
        let content =
            if let Some(old) = generation_receipt_target(bytes).map_err(|e| e.to_string())? {
                let new = *relocated
                    .get(&old.offset)
                    .ok_or("retained generation receipt lost its target")?;
                receipt = relocate_generation_receipt(bytes, new).map_err(|e| e.to_string())?;
                receipt.as_slice()
            } else {
                bytes
            };
        let written = target.append(content).map_err(|e| e.to_string())?;
        if bytes.starts_with(b"RELVGEN1") {
            relocated.insert(object.offset, written);
        }
        Ok(())
    })?;
    target.sync().map_err(|e| e.to_string())
}

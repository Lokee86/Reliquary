//! Disposable bounded external ordering of immutable accepted event effects.
//! One journal scan, fixed-size sorted runs, then two-way merges. Scratch bytes
//! are never authority; each query rebuilds them from its accepted prefix.
use super::{
    FreshnessStore, JournalEntry, decode, encode, event_receipts_equal, event_sort_key,
    for_each_event, receipt_bytes,
};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

const RUN_BYTES: usize = 256 * 1024;

#[cfg(test)]
mod tests {
    use super::*;
    fn event(n: usize) -> JournalEntry {
        JournalEntry::Event {
            event_id: format!("sort-{n:08}"),
            turn: (n / 7) as u64,
            provenance: super::super::FreshnessEventProvenance {
                producer_kind: 2,
                source_id: format!("sort-{n}"),
                origin_owner: [7; 16],
                accepted_turn: (n / 7) as u64,
                policy_version: 1,
            },
            proof: crate::FreshnessEventProof {
                graph_version: 0,
                community_generation: None,
                community_graph_version: None,
            },
            effects: std::collections::BTreeMap::from([(crate::MemoryId([1; 32]), 25)]),
            records: std::collections::BTreeMap::new(),
        }
    }
    #[test]
    fn multi_run_sort_is_bounded_deduplicates_and_cleans_up() {
        let mut runs = Runs::new();
        let directory = runs.directory.clone();
        for n in (0..20000).rev() {
            runs.push(&event(n)).unwrap();
            assert!(runs.bytes < RUN_BYTES);
            assert!(runs.levels.len() <= 16);
        }
        runs.push(&event(0)).unwrap();
        let mut expected = 0;
        runs.visit(|value| {
            assert_eq!(event_sort_key(value), event_sort_key(&event(expected)));
            expected += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(expected, 20000);
        assert!(!directory.exists());
    }
    #[test]
    fn conflicting_cross_run_identity_and_partial_spill_fail_closed() {
        let mut runs = Runs::new();
        let directory = runs.directory.clone();
        runs.push(&event(0)).unwrap();
        for n in 1..3000 {
            runs.push(&event(n)).unwrap();
        }
        let mut forged = event(0);
        if let JournalEntry::Event { effects, .. } = &mut forged {
            *effects.values_mut().next().unwrap() = 20;
        }
        let result = runs.push(&forged).and_then(|_| runs.visit(|_| Ok(())));
        assert!(result.is_err());
        assert!(!directory.exists());
        let mut frame = Vec::new();
        write_event(&mut frame, &event(0)).unwrap();
        for end in 1..frame.len() {
            assert!(read_event(&mut &frame[..end]).is_err(), "prefix {end}");
        }
        assert_eq!(read_event(&mut &frame[..]).unwrap(), Some(event(0)));
    }
}

struct Runs {
    directory: PathBuf,
    next: usize,
    levels: Vec<Option<PathBuf>>,
    buffer: Vec<JournalEntry>,
    bytes: usize,
}
impl Drop for Runs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
impl Runs {
    fn new() -> Self {
        Self {
            directory: std::env::temp_dir()
                .join(format!("reliquary-freshness-sort-{}", uuid::Uuid::new_v4())),
            next: 0,
            levels: Vec::new(),
            buffer: Vec::new(),
            bytes: 0,
        }
    }
    fn path(&mut self) -> Result<PathBuf, String> {
        fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let path = self.directory.join(format!("{}.run", self.next));
        self.next += 1;
        Ok(path)
    }
    fn push(&mut self, event: &JournalEntry) -> Result<(), String> {
        let bytes = receipt_bytes(event);
        if !self.buffer.is_empty() && self.bytes.saturating_add(bytes) > RUN_BYTES {
            self.flush()?;
        }
        self.buffer.push(event.clone());
        self.bytes = self.bytes.saturating_add(bytes);
        if self.bytes >= RUN_BYTES {
            self.flush()?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.buffer.sort_by_key(event_sort_key);
        let mut path = self.path()?;
        {
            let mut output = BufWriter::new(File::create(&path).map_err(|e| e.to_string())?);
            emit_unique(self.buffer.iter().cloned(), |event| {
                write_event(&mut output, event)
            })?;
            output.flush().map_err(|e| e.to_string())?;
        }
        self.buffer.clear();
        self.bytes = 0;
        let mut level = 0;
        loop {
            if level == self.levels.len() {
                self.levels.push(None);
            }
            if let Some(previous) = self.levels[level].take() {
                let merged = self.path()?;
                merge(&previous, &path, &merged)?;
                fs::remove_file(previous).map_err(|e| e.to_string())?;
                fs::remove_file(path).map_err(|e| e.to_string())?;
                path = merged;
                level += 1;
            } else {
                self.levels[level] = Some(path);
                break;
            }
        }
        Ok(())
    }
    fn visit(
        mut self,
        mut visitor: impl FnMut(&JournalEntry) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.levels.is_empty() {
            self.buffer.sort_by_key(event_sort_key);
            return emit_unique(self.buffer.iter().cloned(), &mut visitor);
        }
        self.flush()?;
        let paths: Vec<_> = self.levels.iter_mut().filter_map(Option::take).collect();
        let mut current: Option<PathBuf> = None;
        for path in paths {
            current = Some(if let Some(previous) = current {
                let merged = self.path()?;
                merge(&previous, &path, &merged)?;
                fs::remove_file(previous).map_err(|e| e.to_string())?;
                fs::remove_file(path).map_err(|e| e.to_string())?;
                merged
            } else {
                path
            });
        }
        if let Some(path) = current {
            let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
            while let Some(event) = read_event(&mut reader)? {
                visitor(&event)?;
            }
        }
        Ok(())
    }
}
fn emit_unique(
    events: impl IntoIterator<Item = JournalEntry>,
    mut visitor: impl FnMut(&JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    let mut previous: Option<JournalEntry> = None;
    for event in events {
        if let Some(old) = &previous {
            if event_sort_key(old) == event_sort_key(&event) {
                if !event_receipts_equal(old, &event) {
                    return Err("conflicting canonical Freshness event identities".into());
                }
                continue;
            }
        }
        visitor(&event)?;
        previous = Some(event);
    }
    Ok(())
}
fn write_event(writer: &mut impl Write, event: &JournalEntry) -> Result<(), String> {
    let payload = encode([0; 16], event);
    writer
        .write_all(&(payload.len() as u64).to_le_bytes())
        .and_then(|_| writer.write_all(&payload))
        .map_err(|e| e.to_string())
}
fn read_event(reader: &mut impl Read) -> Result<Option<JournalEntry>, String> {
    let mut length = [0u8; 8];
    // EOF is valid only before a frame, never halfway through one.
    if reader.read(&mut length[..1]).map_err(|e| e.to_string())? == 0 {
        return Ok(None);
    }
    reader
        .read_exact(&mut length[1..])
        .map_err(|e| e.to_string())?;
    let length = usize::try_from(u64::from_le_bytes(length))
        .map_err(|_| "Freshness spill length overflow")?;
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload).map_err(|e| e.to_string())?;
    let (_, event) = decode(&payload)?;
    if !matches!(event, JournalEntry::Event { .. }) {
        return Err("non-event in Freshness spill".into());
    }
    Ok(Some(event))
}
fn merge(left: &PathBuf, right: &PathBuf, destination: &PathBuf) -> Result<(), String> {
    let mut left = BufReader::new(File::open(left).map_err(|e| e.to_string())?);
    let mut right = BufReader::new(File::open(right).map_err(|e| e.to_string())?);
    let mut output = BufWriter::new(File::create(destination).map_err(|e| e.to_string())?);
    let mut a = read_event(&mut left)?;
    let mut b = read_event(&mut right)?;
    while a.is_some() || b.is_some() {
        let take_left = match (&a, &b) {
            (Some(a), Some(b)) => event_sort_key(a) <= event_sort_key(b),
            (Some(_), None) => true,
            _ => false,
        };
        if let (Some(left_event), Some(right_event)) = (&a, &b) {
            if event_sort_key(left_event) == event_sort_key(right_event) {
                if !event_receipts_equal(left_event, right_event) {
                    return Err("conflicting canonical Freshness event identities".into());
                }
                write_event(&mut output, left_event)?;
                a = read_event(&mut left)?;
                b = read_event(&mut right)?;
                continue;
            }
        }
        if take_left {
            write_event(&mut output, a.as_ref().unwrap())?;
            a = read_event(&mut left)?;
        } else {
            write_event(&mut output, b.as_ref().unwrap())?;
            b = read_event(&mut right)?;
        }
    }
    output.flush().map_err(|e| e.to_string())
}
pub(super) fn ordered_events(
    store: &FreshnessStore,
    through: u64,
    before: Option<(&str, bool)>,
    graph: u64,
    extra: &[JournalEntry],
    visitor: impl FnMut(&JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    let mut runs = Runs::new();
    let mut consider = |event: &JournalEntry| {
        let JournalEntry::Event {
            turn,
            event_id,
            proof,
            ..
        } = event
        else {
            return Ok(());
        };
        if *turn > through
            || proof.graph_version > graph
            || (*turn == through
                && before.is_some_and(|(key, inclusive)| {
                    if inclusive {
                        event_id.as_str() > key
                    } else {
                        event_id.as_str() >= key
                    }
                }))
        {
            return Ok(());
        }
        runs.push(event)
    };
    store.scan_entries(|entry| for_each_event(&entry, &mut consider))?;
    for event in extra {
        consider(event)?;
    }
    runs.visit(visitor)
}

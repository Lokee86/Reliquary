use anyhow::Result;
use flate2::read::GzDecoder;
use reliquary_memory::Cva;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize)]
struct ConversationTitleRow {
    conversation_id: String,
    title: String,
}

pub fn run_conversation_titles(source: &Path, rel: &Path) -> Result<()> {
    let file = File::open(source)?;
    let reader: Box<dyn Read> = if source.extension().is_some_and(|ext| ext == "gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut csv = csv::Reader::from_reader(reader);
    let mut archive = Cva::open(rel)?;
    let existing = archive
        .conversation_summaries()
        .into_iter()
        .map(|summary| summary.conversation_id)
        .collect::<HashSet<_>>();
    let mut matched = 0_usize;
    let mut updates = 0_usize;
    let mut skipped_empty = 0_usize;

    for row in csv.deserialize::<ConversationTitleRow>() {
        let row = row?;
        let title = row.title.trim();
        if title.is_empty() {
            skipped_empty += 1;
            continue;
        }
        if !existing.contains(&row.conversation_id) {
            continue;
        }
        matched += 1;
        updates +=
            usize::from(archive.set_conversation_title(&row.conversation_id, title.to_owned())?);
    }

    archive.sync()?;
    println!(
        "conversation-title migration: matched={} updates={} skipped_empty={} rel={}",
        matched,
        updates,
        skipped_empty,
        rel.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn conversation_title_migration_backfills_gzipped_catalog() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("reliquary-title-migration-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let rel = dir.join("project.prj.rel");
        let catalog = dir.join("canonical_conversations.csv.gz");

        let mut cva = Cva::create_project(&rel).unwrap();
        cva.append_node(
            "n1".into(),
            "conversation-1".into(),
            None,
            "user".into(),
            1,
            "hello",
        )
        .unwrap();
        cva.sync().unwrap();
        drop(cva);

        let mut gzip = GzEncoder::new(File::create(&catalog).unwrap(), Compression::default());
        gzip.write_all(b"conversation_id,title\nconversation-1,Imported title\n")
            .unwrap();
        gzip.finish().unwrap();

        run_conversation_titles(&catalog, &rel).unwrap();
        let reopened = Cva::open(&rel).unwrap();
        assert_eq!(
            reopened.conversation_summaries()[0].title.as_deref(),
            Some("Imported title")
        );
    }
}

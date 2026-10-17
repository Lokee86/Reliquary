use crate::args::ArchiveCommand;
use crate::util::{hex32, parse_fragment_id};
use anyhow::{Result, anyhow};
use reliquary_memory::Cva;
use std::collections::BTreeMap;

pub fn run(command: ArchiveCommand) -> Result<()> {
    match command {
        ArchiveCommand::Conversations { cva } => conversations(&cva),
        ArchiveCommand::Show {
            cva,
            conversation,
            branch,
        } => show(&cva, &conversation, &branch),
        ArchiveCommand::Fragments { cva, conversation } => fragments(&cva, conversation.as_deref()),
        ArchiveCommand::Fragment { cva, id } => fragment(&cva, &id),
        ArchiveCommand::Tombstone { cva, conversation } => tombstone(&cva, &conversation),
        ArchiveCommand::Lifecycle { cva, conversation } => lifecycle(&cva, &conversation),
    }
}

fn conversations(path: &std::path::Path) -> Result<()> {
    let cva = Cva::open(path)?;
    let mut conversations: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for branch in cva.branches() {
        conversations
            .entry(branch.conversation_id.clone())
            .or_default()
            .push(branch);
    }
    for (conversation, mut branches) in conversations {
        branches.sort_by(|left, right| left.id.cmp(&right.id));
        println!("{conversation}");
        for branch in branches {
            println!(
                "  {}  leaf={} canonical={}",
                branch.id, branch.leaf_node_id, branch.canonical
            );
        }
    }
    Ok(())
}

fn show(path: &std::path::Path, conversation: &str, branch: &str) -> Result<()> {
    let mut cva = Cva::open(path)?;
    let turns = cva.branch_turns(conversation, branch)?;
    println!(
        "conversation={conversation} branch={branch} turns={}",
        turns.len()
    );
    for turn in turns {
        println!("\n[{}] {} @ {}", turn.node_id, turn.role, turn.timestamp_ns);
        println!("{}", turn.content);
    }
    Ok(())
}

fn fragments(path: &std::path::Path, conversation: Option<&str>) -> Result<()> {
    let cva = Cva::open(path)?;
    for fragment in cva.fragments() {
        if conversation.is_some_and(|value| value != fragment.conversation_id) {
            continue;
        }
        println!(
            "{}\t{}\t{}..{}",
            hex32(&fragment.id.0),
            fragment.conversation_id,
            fragment.start_node_id,
            fragment.end_node_id
        );
    }
    Ok(())
}

fn tombstone(path: &std::path::Path, conversation: &str) -> Result<()> {
    let mut cva = Cva::open(path)?;
    let published = cva.tombstone_conversation(conversation)?;
    cva.sync()?;
    let status = if published {
        "published"
    } else {
        "already-tombstoned"
    };
    println!("conversation={conversation} lifecycle=tombstoned result={status}");
    Ok(())
}

fn lifecycle(path: &std::path::Path, conversation: &str) -> Result<()> {
    let cva = Cva::open(path)?;
    let state = cva
        .conversation_lifecycle_state(conversation)
        .map_err(|error| anyhow!("{error:?}"))?;
    match state {
        reliquary_memory::owner_lifecycle::LifecycleState::Active => {
            println!("conversation={conversation} lifecycle=active");
        }
        reliquary_memory::owner_lifecycle::LifecycleState::Tombstoned(at) => {
            println!(
                "conversation={conversation} lifecycle=tombstoned global_version={:?} archive_version={:?}",
                at.global_version, at.domain_version
            );
        }
    }
    Ok(())
}

fn fragment(path: &std::path::Path, value: &str) -> Result<()> {
    let id = parse_fragment_id(value)?;
    let mut cva = Cva::open(path)?;
    let metadata = cva
        .fragments()
        .into_iter()
        .find(|fragment| fragment.id == id)
        .ok_or_else(|| anyhow!("fragment not found"))?;
    println!("id: {}", hex32(&metadata.id.0));
    println!("conversation: {}", metadata.conversation_id);
    println!(
        "range: {}..{}",
        metadata.start_node_id, metadata.end_node_id
    );
    println!("\n{}", cva.fragment_text(id)?);
    Ok(())
}

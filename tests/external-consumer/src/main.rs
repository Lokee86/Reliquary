use reliquary_memory::{
    Gateway, HostCredentialBinding, MemoryHistoryLookup, MemoryId, OwnerGrant, Phylactery, Rel,
    TrustedHostAdmission,
};
use std::{env, process::ExitCode};

fn quoted(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn inspect(paths: &[String]) -> Result<String, String> {
    if paths.len() != 4 {
        return Err("expected rel-lantern rel-cascade phy-reader-a phy-reader-b".into());
    }
    let lantern = Rel::open(&paths[0]).map_err(|error| error.to_string())?;
    let cascade = Rel::open(&paths[1]).map_err(|error| error.to_string())?;
    let phy_a = Phylactery::open(&paths[2]).map_err(|error| error.to_string())?;
    let phy_b = Phylactery::open(&paths[3]).map_err(|error| error.to_string())?;
    let ids = [
        lantern.owner_id(),
        cascade.owner_id(),
        phy_a.owner_id(),
        phy_b.owner_id(),
    ];
    let gateway = Gateway::new();
    let lantern_id = gateway
        .mount_rel(&lantern, true)
        .map_err(|error| error.to_string())?;
    let cascade_id = gateway
        .mount_rel(&cascade, true)
        .map_err(|error| error.to_string())?;
    let phy_a_id = gateway
        .mount_phylactery(&phy_a, false)
        .map_err(|error| error.to_string())?;
    let phy_b_id = gateway
        .mount_phylactery(&phy_b, false)
        .map_err(|error| error.to_string())?;
    let operator_token = "external-g4-operator-token-000000000000000000000";
    let viewer_token = "external-g4-viewer-token-000000000000000000000000";
    let outsider_token = "external-g4-outsider-token-00000000000000000000000";
    let (adapter, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "operator",
            operator_token,
            [
                OwnerGrant::ManageRel(lantern_id),
                OwnerGrant::ManageRel(cascade_id),
                OwnerGrant::ViewPhylactery(phy_a_id),
            ],
        ),
        HostCredentialBinding::new(
            "viewer",
            viewer_token,
            [
                OwnerGrant::ViewRel(lantern_id),
                OwnerGrant::ViewRel(cascade_id),
                OwnerGrant::ViewPhylactery(phy_a_id),
            ],
        ),
        HostCredentialBinding::new(
            "outsider",
            outsider_token,
            [OwnerGrant::ViewRel(cascade_id)],
        ),
    ])
    .install(&gateway)
    .map_err(|error| error.to_string())?;
    let operator = adapter
        .authenticate(operator_token)
        .map_err(|error| error.to_string())?;
    let viewer = adapter
        .authenticate(viewer_token)
        .map_err(|error| error.to_string())?;
    let outsider = adapter
        .authenticate(outsider_token)
        .map_err(|error| error.to_string())?;

    let lantern_management = operator
        .workspace_management(lantern_id)
        .map_err(|error| error.to_string())?;
    let cascade_management = operator
        .workspace_management(cascade_id)
        .map_err(|error| error.to_string())?;
    let lantern_page = lantern_management
        .conversations(0, 8)
        .map_err(|error| error.to_string())?;
    let cascade_page = cascade_management
        .conversations(0, 4)
        .map_err(|error| error.to_string())?;
    if lantern_page.total != 8 || cascade_page.total != 4 {
        return Err("cohort conversation counts differ from manifest".into());
    }
    let lantern_turns: u64 = lantern_page
        .items
        .iter()
        .map(|item| item.turn_count as u64)
        .sum();
    let cascade_turns: u64 = cascade_page
        .items
        .iter()
        .map(|item| item.turn_count as u64)
        .sum();
    if lantern_turns != 32 || cascade_turns != 16 {
        return Err("cohort turn counts differ from manifest".into());
    }

    let lantern_memories = lantern_management
        .memories(0, 8)
        .map_err(|error| error.to_string())?;
    let cascade_memories = cascade_management
        .memories(0, 8)
        .map_err(|error| error.to_string())?;
    let memory = lantern_memories
        .items
        .first()
        .ok_or("Lantern Memory missing")?;
    let memory_id = MemoryId(parse_memory_id(&memory.id)?);
    let direct = lantern_management
        .memory(memory_id)
        .map_err(|error| error.to_string())?;
    if direct.content != memory.content || direct.title != memory.title {
        return Err("direct Memory differs from bounded list result".into());
    }
    let provenance = lantern_management
        .provenance(memory_id)
        .map_err(|error| error.to_string())?;
    if provenance.owner_id != lantern_id.to_string() || provenance.source_node_id.is_empty() {
        return Err("Memory provenance is incomplete".into());
    }
    let history = lantern
        .memory_history(memory_id)
        .map_err(|error| error.to_string())?;
    if !matches!(history, MemoryHistoryLookup::Found(_)) {
        return Err("Memory history missing after independent open".into());
    }
    let proof = lantern
        .current_historical_proof()
        .map_err(|error| error.to_string())?;

    if viewer
        .workspace_management(lantern_id)
        .map_err(|error| error.to_string())?
        .create_memory(
            "denied".into(),
            "denied".into(),
            "fact".into(),
            "note".into(),
            1,
        )
        .is_ok()
    {
        return Err("viewer unexpectedly wrote a Memory".into());
    }
    if outsider.workspace_management(lantern_id).is_ok() {
        return Err("foreign REL unexpectedly opened".into());
    }
    if viewer.phylactery_info(phy_b_id).is_ok() {
        return Err("viewer crossed into principal B PHY".into());
    }
    if operator
        .phylactery_info(phy_a_id)
        .map_err(|error| error.to_string())?
        .owner_id
        != ids[2]
    {
        return Err("authorized PHY identity changed".into());
    }
    control
        .revoke(viewer_token)
        .map_err(|error| error.to_string())?;
    if viewer
        .workspace_management(lantern_id)
        .and_then(|management| management.conversations(0, 1).map(|_| ()))
        .is_ok()
    {
        return Err("revoked viewer retained read access".into());
    }

    lantern_management
        .quiesce()
        .map_err(|error| error.to_string())?;
    cascade_management
        .quiesce()
        .map_err(|error| error.to_string())?;
    if lantern_management
        .verify_quiesced()
        .map_err(|error| error.to_string())?
        .owner_id
        != ids[0]
    {
        return Err("quiesced Lantern identity changed".into());
    }
    if cascade_management
        .verify_quiesced()
        .map_err(|error| error.to_string())?
        .owner_id
        != ids[1]
    {
        return Err("quiesced Cascade identity changed".into());
    }
    lantern_management
        .reopen()
        .map_err(|error| error.to_string())?;
    cascade_management
        .reopen()
        .map_err(|error| error.to_string())?;

    Ok(format!(
        "{{\"rel_lantern\":{{\"owner_id\":{},\"conversations\":{},\"turns\":{},\"memories\":{},\"provenance_source_node\":{},\"history_digest\":{}}},\"rel_cascade\":{{\"owner_id\":{},\"conversations\":{},\"turns\":{},\"memories\":{}}},\"phy_a\":{{\"owner_id\":{},\"memory_count\":{}}},\"phy_b\":{{\"owner_id\":{},\"memory_count\":{}}}}}",
        quoted(&ids[0].to_string()),
        lantern_page.total,
        lantern_turns,
        lantern_memories.total,
        quoted(&provenance.source_node_id),
        quoted(&proof.digest),
        quoted(&ids[1].to_string()),
        cascade_page.total,
        cascade_turns,
        cascade_memories.total,
        quoted(&ids[2].to_string()),
        phy_a
            .info()
            .map_err(|error| error.to_string())?
            .memory_count,
        quoted(&ids[3].to_string()),
        phy_b
            .info()
            .map_err(|error| error.to_string())?
            .memory_count
    ))
}

fn parse_memory_id(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("Memory ID is not 64 hex characters".into());
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match inspect(&args) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{{\"error\":{}}}", quoted(&error));
            ExitCode::FAILURE
        }
    }
}

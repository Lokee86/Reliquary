//! Temporary offline admission only; concrete owners still validate payloads.
//! Remove with the one-time upgrader after canonical corpus promotion.
use super::StorageUpgradeError;

pub(super) fn validate(payload: &[u8]) -> Result<(), StorageUpgradeError> {
    if !(payload.starts_with(b"CVA") || payload.starts_with(b"CVD") || payload.starts_with(b"PRJ"))
    {
        return Ok(());
    }
    let mut identity = [0u8; 8];
    let n = payload.len().min(8);
    identity[..n].copy_from_slice(&payload[..n]);
    if !KNOWN.iter().any(|known| *known == &identity) {
        return Err(StorageUpgradeError::UnknownAuthority(identity));
    }
    Ok(())
}

// Explicit families audited in the current durable-family inventory.
// This list grants no owner validity and introduces no runtime dispatch.
const KNOWN: &[&[u8; 8]] = &[
    // global_ticket
    b"CVAVERS1",
    b"CVAVERS2",
    // archive_publication
    b"CVAAFMT2",
    b"CVAAREC1",
    b"CVAARP02",
    // publication_envelope
    b"CVAPUB01",
    // archive_content
    b"CVACONT1",
    // archive_node
    b"CVANODE1",
    b"CVANODE2",
    b"CVANOD02",
    // archive_branch
    b"CVABRCH1",
    b"CVABRC02",
    // archive_conversation
    b"CVACONV1",
    b"CVACONV2",
    b"CVACONV3",
    b"CVACON04",
    // archive_terminal
    b"CVACTMB1",
    b"CVACTM02",
    // archive_fragment
    b"CVAFRAG1",
    b"CVAFRA02",
    // archive_episode
    b"CVAEPIS1",
    b"CVAEPI02",
    // archive_file
    b"CVAFILE1",
    b"CVAFIL02",
    // archive_turn_attachment
    b"CVATURN1",
    b"CVATURN2",
    b"CVATUR03",
    // archive_file_memory_link
    b"CVAFMEM1",
    b"CVAFME02",
    // archive_import_receipt
    b"CVAIMPT1",
    b"CVAIMP02",
    // archive_prune
    b"CVAPRND1",
    // conversation_compaction
    b"CVACMP1\0",
    // interaction_stream
    b"CVAISTR1",
    b"CVAISTR2",
    // echo
    b"CVAECHO1",
    // project_correlation
    b"PRJCOR01",
    b"PRJCOR02",
    // project_file_binding
    b"PRJFILE1",
    // memory_format
    b"CVAMEMF2",
    // memory_body
    b"CVAMBDY1",
    // memory_revision
    b"CVAMEMR2",
    b"CVAMEMR3",
    b"CVAMEMR4",
    b"CVAMEMR5",
    b"CVAMEMR6",
    b"CVAMEMR7",
    b"CVAMEMR8",
    b"CVAMC001",
    b"CVAMD001",
    b"CVAMC002",
    b"CVAMD002",
    b"CVAMT002",
    // memory_publication
    b"CVAMEMV1",
    // memory_terminal
    b"CVAMEMT1",
    // memory_routing
    b"CVAMRTE1",
    b"CVAMRTE2",
    // memory_prune
    b"CVAMPRN1",
    // processing_epoch
    b"CVAPEP01",
    // entity
    b"CVAENTF1",
    b"CVAENTR1",
    b"CVAENTR2",
    b"CVAENTV1",
    b"CVAENTT1",
    b"CVAENTT2",
    b"CVAENC01",
    b"CVAEND01",
    b"CVAENC02",
    b"CVAEND02",
    b"CVAETD02",
    b"CVAETM02",
    // entity_prune
    b"CVAEPRN1",
    // entity_resolution
    b"CVAERS01",
    b"CVAERS02",
    b"CVAERS03",
    b"CVAERL01",
    // relationship
    b"CVARELF1",
    b"CVARELR1",
    b"CVARELV1",
    b"CVARELT1",
    b"CVARNC01",
    b"CVARND01",
    b"CVARNC02",
    b"CVARND02",
    b"CVARTD02",
    // relationship_prune
    b"CVARPRN1",
    // graph
    b"CVAGFMT1",
    b"CVAGNODE",
    b"CVAGNOD2",
    b"CVAGMUT1",
    b"CVAGMUT2",
    b"CVAGBAT1",
    b"CVAGBAT2",
    b"CVAGVER1",
    b"CVAGCM01",
    b"CVAGCB01",
    b"CVAGCM02",
    b"CVAGCB02",
    // graph_rewrite_baseline
    b"CVAGBL01",
    // community_membership
    b"CVACOMM1",
    // community_name
    b"CVACNAM1",
    // dream_pairs
    b"CVADRP01",
    b"CVADRM01",
    b"CVADRP02",
    // dream_freshness
    b"CVDREAM1",
    b"CVDREAM2",
    // legacy_dream_cooldown
    b"CVADREM1",
    b"CVADREM2",
    // freshness_policy
    b"CVAFRP01",
    // freshness_events
    b"CVAFRS02",
    // freshness_import_settlement
    b"CVAFRS03",
    b"CVAFRS04",
    b"CVAFRS05",
    b"CVAFRS06",
    // freshness_legacy_activation
    b"CVAFSA01",
    // freshness_rewrite_baseline
    b"CVAFRB01",
    // import_epoch
    b"CVAIEPS1",
    b"CVAIEPC1",
    // import_legacy_epoch
    b"CVAILEG1",
    b"CVAILEG2",
    // insomnia_format
    b"CVAINSF1",
    // insomnia_work
    b"CVAINSW1",
    b"CVAINSA1",
    // insomnia_completion
    b"CVAINSC1",
    b"CVAINSC2",
    b"CVAINSC3",
    b"CVAINSC4",
    b"CVAINSC5",
    b"CVAINSC6",
    b"CVAINSC7",
    b"CVAINSC8",
    // external_index
    b"CVAXIDX1",
    b"CVAXIDX2",
    // external_publication
    b"CVAXMEM1",
    b"CVAXMEM2",
    // ego_compact_history
    b"CVAECR01",
    b"CVAEDL01",
    b"CVAETM01",
    // ego_identity
    b"CVAEIDN1",
    b"CVAEIDN2",
    b"CVAEIDA1",
    // ego_personality
    b"CVAEPER1",
    // ego_anchor
    b"CVAEANC1",
    // ego_synthesis
    b"CVAESYN1",
    // packed_vectors
    b"CVAPVFM1",
    b"CVAPVEC1",
    b"CVAPVRN1",
    // memory_vectors
    b"CVAMVFM1",
    b"CVAMVEC1",
    // archive_vectors
    b"CVAAVFM1",
    b"CVAAVEC1",
    // compatibility_profile
    b"CVACPFM1",
    b"CVACPRO1",
    b"CVACPRT1",
    // vector_generation
    b"CVAVGFM2",
    b"CVAVGEN2",
    b"CVAVGRC2",
    // rel_metadata
    b"CVARELM1",
    // phy_profile
    b"CVAPHYP1",
    // owner_maintenance
    b"CVAHPR01",
    // rewrite_lineage
    b"CVARGN01",
    // legacy_workspace
    b"CVAWKFM1",
    b"CVAWKSP1",
];

use crate::{evidence_encoding::parse_hex, memory::observe_memory_id_page};
use reliquary_owner_session::{
    CurrentOwnerKind, MAX_MEMORY_ID_CURSOR_BYTES, MemoryId, ReadOnlyCurrentOwnerObserver,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) const CONTRACT_COMMIT: &str = "0f824a09fe01f5ff327ab56f3296f36d8f4d7c87";
pub(crate) const PROBE_SCHEMA_ID: &str = "reliquary-d02-current-reader-probe-v3";
const FIXTURE_FAMILIES: [&str; 5] = [
    "fixture.archive_turn_revision.v1",
    "fixture.ego_anchor.v1",
    "fixture.freshness_event.v1",
    "fixture.memory_revision.v1",
    "fixture.vector_probe.v1",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateKind {
    Rel,
    Phy,
}

impl CandidateKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "rel" | "REL" => Ok(Self::Rel),
            "phy" | "PHY" => Ok(Self::Phy),
            _ => Err("kind must be rel or phy".into()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Rel => "REL",
            Self::Phy => "PHY",
        }
    }

    pub(crate) fn matches(self, observed: CurrentOwnerKind) -> bool {
        matches!(
            (self, observed),
            (Self::Rel, CurrentOwnerKind::Rel) | (Self::Phy, CurrentOwnerKind::Phy)
        )
    }
}

pub fn probe_candidate(kind: CandidateKind, path: &Path) -> Value {
    probe_candidate_page(kind, path, None)
}

pub fn probe_candidate_page(
    expected_kind: CandidateKind,
    path: &Path,
    memory_cursor: Option<&str>,
) -> Value {
    let mut observer = match ReadOnlyCurrentOwnerObserver::open(path) {
        Ok(observer) => observer,
        Err(reason) => return incomplete_open_report(expected_kind, reason),
    };
    let observation = observer.observation().clone();
    let kind_matches = expected_kind.matches(observation.kind);
    let memory = if kind_matches {
        observe_memory_id_page(&mut observer, memory_cursor)
    } else {
        Err("requested_candidate_kind_mismatch".into())
    };
    let finalization = match observer.finalize() {
        Ok(finalization) => finalization,
        Err(reason) => {
            return incomplete_observation_report(
                expected_kind,
                &observation,
                None,
                None,
                None,
                Some(reason),
            );
        }
    };

    let (memory, page_error) = match memory {
        Ok(memory) => (Some(memory), None),
        Err(reason) => (None, Some(reason)),
    };
    let reason = if !finalization.candidate_unchanged {
        Some("candidate_changed_during_observation".to_string())
    } else {
        page_error
    };
    incomplete_observation_report(
        expected_kind,
        &observation,
        memory,
        Some(finalization.file_sha256_after),
        Some(finalization),
        reason,
    )
}

fn incomplete_open_report(kind: CandidateKind, reason: String) -> Value {
    json!({
        "schema_id": PROBE_SCHEMA_ID,
        "contract_commit": CONTRACT_COMMIT,
        "evidence_scope": "reader_probe_not_parity_evidence",
        "status": "INCOMPLETE",
        "candidate_kind": kind.label(),
        "current_only_api": true,
        "same_handle_observation": false,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "path_matches_open_file_before": null,
        "path_matches_open_file_after": null,
        "owner": null,
        "target_clocks": null,
        "native_historical_proof": null,
        "historical_proof_used_as_export_gate": false,
        "memory_observation": null,
        "semantic_rows": [],
        "semantic_row_count": "0",
        "semantic_export_state": "INCOMPLETE",
        "cut_coverage": {
            "latest_allocated_global_version": "INCOMPLETE",
            "historical_cut_inventory": "INCOMPLETE"
        },
        "family_coverage": family_coverage(),
        "reason": reason
    })
}

fn incomplete_observation_report(
    expected_kind: CandidateKind,
    observation: &reliquary_owner_session::CurrentOwnerObservation,
    memory: Option<Value>,
    file_sha256_after: Option<[u8; 32]>,
    finalization: Option<reliquary_owner_session::CurrentOwnerFinalization>,
    reason: Option<String>,
) -> Value {
    let candidate_unchanged = finalization.as_ref().map(|value| value.candidate_unchanged);
    let path_matches_open_file_after = finalization
        .as_ref()
        .map(|value| value.path_matches_open_file_after);
    json!({
        "schema_id": PROBE_SCHEMA_ID,
        "contract_commit": CONTRACT_COMMIT,
        "evidence_scope": "reader_probe_not_parity_evidence",
        "status": "INCOMPLETE",
        "candidate_kind": expected_kind.label(),
        "observed_owner_kind": observation.kind.as_str(),
        "requested_kind_matches_observed": expected_kind.matches(observation.kind),
        "current_only_api": true,
        "same_handle_observation": true,
        "shared_read_only_lease_held_through_finalization": true,
        "candidate_sha256_before": hex(&observation.file_sha256_before),
        "candidate_sha256_after": file_sha256_after.map(|value| hex(&value)),
        "candidate_unchanged": candidate_unchanged,
        "path_matches_open_file_before": true,
        "path_matches_open_file_after": path_matches_open_file_after,
        "physical_file_identity_unchanged": finalization.as_ref().map(|value| value.physical_file_identity_unchanged),
        "owner": {
            "kind": observation.kind.as_str(),
            "owner_uuid": hex(&observation.owner_id),
            "scope": format!("{:?}", observation.scope).to_ascii_lowercase(),
            "identity_is_principal_authorization": false
        },
        "target_clocks": {
            "latest_allocated_global_version": observation.latest_allocated_global_version.to_string(),
            "transaction_time_ns_at_latest_allocated_version": observation.transaction_time_ns_at_latest_allocated_version.map(|value| value.to_string()),
            "memory_version": observation.memory_version.to_string(),
            "history_generation_sha256": hex(&observation.history_generation),
            "accepted_global_boundary_inventory": "INCOMPLETE"
        },
        "native_historical_proof": null,
        "historical_proof_used_as_export_gate": false,
        "memory_observation": memory,
        "semantic_rows": [],
        "semantic_row_count": "0",
        "semantic_export_state": "INCOMPLETE",
        "total_current_owner_replay_bounded": false,
        "total_current_owner_replay_note": "The normal current-only owner open replays the full owner before these bounded page APIs; this probe does not claim replay is size-bounded.",
        "cut_coverage": {
            "latest_allocated_global_version": "observed_not_an_accepted_cut_inventory",
            "historical_cut_inventory": "INCOMPLETE",
            "reason": "No exhaustive accepted-boundary inventory is exported."
        },
        "family_coverage": family_coverage(),
        "reason": reason.unwrap_or_default()
    })
}

fn family_coverage() -> Vec<Value> {
    FIXTURE_FAMILIES
        .iter()
        .map(|family| {
            let reason = match *family {
                "fixture.memory_revision.v1" => "One bounded current Memory-ID page only; revisions, bodies, lifecycle, provenance, complete history and product-family mapping are INCOMPLETE.",
                _ => "PG-C01 family schema is synthetic; this observer exports no product mapping for this family."
            };
            json!({
                "schema_family_id": family,
                "status": "INCOMPLETE",
                "reason": reason
            })
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub struct CliArgs {
    pub kind: CandidateKind,
    pub candidate: PathBuf,
    pub output: Option<PathBuf>,
    pub memory_cursor: Option<String>,
    pub archive_cut: Option<u64>,
    pub archive_binding_cut: Option<u64>,
    pub memory_history: Option<(MemoryId, u64)>,
}

pub fn parse_cli_args<I, S>(args: I) -> Result<CliArgs, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let mut kind = None;
    let mut candidate = None;
    let mut output = None;
    let mut memory_cursor = None;
    let mut archive_cut = None;
    let mut archive_binding_cut = None;
    let mut memory_history = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--kind" => {
                kind = Some(CandidateKind::parse(
                    &args.next().ok_or("--kind requires a value")?,
                )?)
            }
            "--candidate" => {
                candidate = Some(PathBuf::from(
                    args.next().ok_or("--candidate requires a path")?,
                ))
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output requires a path")?,
                ))
            }
            "--memory-cursor" => {
                let value = args.next().ok_or("--memory-cursor requires a value")?;
                if value.len() > MAX_MEMORY_ID_CURSOR_BYTES {
                    return Err("memory cursor exceeds its byte limit".into());
                }
                memory_cursor = Some(value);
            }
            "--memory-history" => {
                let value = args.next().ok_or("--memory-history requires ID:VERSION")?;
                let (id_text, version) = value
                    .split_once(':')
                    .ok_or("--memory-history requires ID:VERSION")?;
                let id = parse_hex::<32>(id_text).ok_or("invalid Memory ID")?;
                let version_number = version
                    .parse::<u64>()
                    .map_err(|_| "invalid Memory version")?;
                if id_text != hex(&id) || version != version_number.to_string() {
                    return Err(
                        "Memory history requires lowercase hex and canonical unsigned decimal"
                            .into(),
                    );
                }
                memory_history = Some((MemoryId(id), version_number));
            }
            "--archive-cut" | "--archive-binding-cut" => {
                let value = args
                    .next()
                    .ok_or("Archive observation requires a version")?;
                let cut = value
                    .parse::<u64>()
                    .map_err(|_| "invalid Archive version")?;
                if value != cut.to_string() {
                    return Err("Archive version must use canonical unsigned decimal".into());
                }
                let selected = if flag == "--archive-cut" {
                    &mut archive_cut
                } else {
                    &mut archive_binding_cut
                };
                if selected.replace(cut).is_some() {
                    return Err("duplicate Archive observation option".into());
                }
            }
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    if [
        archive_cut.is_some(),
        archive_binding_cut.is_some(),
        memory_cursor.is_some(),
        memory_history.is_some(),
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count()
        > 1
    {
        return Err("select only one Archive cut, Memory history cut, or Memory ID cursor".into());
    }
    Ok(CliArgs {
        kind: kind.ok_or("--kind rel|phy is required")?,
        candidate: candidate.ok_or("--candidate PATH is required")?,
        output,
        memory_cursor,
        archive_cut,
        archive_binding_cut,
        memory_history,
    })
}

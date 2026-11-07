use crate::{
    Failure,
    episode::EpisodeInput,
    transfer::{Revision, Scope, decimal, decode_hex, present_option, proof},
};
use reliquary_archive::{ArchivePublicationBinding, ArchivePublicationCoordinate};
use reliquary_owner_session::{RelSession, RelWriter};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transfer {
    format: Format,
    owner: String,
    scope: Scope,
    history_generation: String,
    events: Vec<Event>,
}

#[derive(Deserialize)]
enum Format {
    #[serde(rename = "reliquary-d01-synthetic-archive-bindings-transfer-v1")]
    VersionedArchive,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    Reserve {
        global_version: String,
        #[serde(deserialize_with = "present_option")]
        transaction_time_ns: Option<String>,
    },
    Apply {
        clock: Clock,
        revision: Revision,
    },
    Episode {
        clock: Clock,
        archive_version: String,
        episode: EpisodeInput,
    },
    Repeat {
        clock: Clock,
        archive_version: String,
        first_applying_archive_version: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Clock {
    global_version: String,
    #[serde(deserialize_with = "present_option")]
    transaction_time_ns: Option<String>,
    bound_reservation: bool,
}

impl Clock {
    fn coordinate(
        &self,
        archive_version: u64,
    ) -> Result<ArchivePublicationCoordinate, &'static str> {
        Ok(ArchivePublicationCoordinate {
            archive_version,
            global_version: decimal(&self.global_version)?,
            transaction_time_ns: self
                .transaction_time_ns
                .as_deref()
                .map(decimal)
                .transpose()?,
            bound_reservation: self.bound_reservation,
        })
    }
}

impl Event {
    fn write(self, writer: &mut RelWriter) -> Result<bool, &'static str> {
        match self {
            Self::Reserve {
                global_version,
                transaction_time_ns,
            } => {
                writer
                    .reserve_archive_publication_version(
                        decimal(&global_version)?,
                        transaction_time_ns.as_deref().map(decimal).transpose()?,
                    )
                    .map_err(|_| "accepted Archive reservation refused by current owner")?;
                Ok(false)
            }
            Self::Apply { clock, revision } => {
                let coordinate = clock.coordinate(revision.archive_version()?)?;
                revision.write(writer, Some(coordinate))?;
                Ok(true)
            }
            Self::Episode {
                clock,
                archive_version,
                episode,
            } => {
                writer
                    .import_versioned_archive_episode(
                        clock.coordinate(decimal(&archive_version)?)?,
                        episode.try_into()?,
                    )
                    .map_err(|_| "accepted Archive Episode refused by current owner")?;
                Ok(true)
            }
            Self::Repeat {
                clock,
                archive_version,
                first_applying_archive_version,
            } => {
                writer
                    .import_archive_repeat(
                        clock.coordinate(decimal(&archive_version)?)?,
                        decimal(&first_applying_archive_version)?,
                    )
                    .map_err(|_| "accepted Archive repeat refused by current owner")?;
                Ok(true)
            }
        }
    }
}

pub fn materialize(bytes: &[u8], staged_file: &Path) -> Result<Value, Failure> {
    let transfer: Transfer = serde_json::from_slice(bytes)
        .map_err(|_| (4, "invalid synthetic Archive binding transfer"))?;
    let Format::VersionedArchive = transfer.format;
    if !(1..=128).contains(&transfer.events.len()) {
        return Err((4, "synthetic Archive publication count is outside bounds"));
    }
    let owner = decode_hex(&transfer.owner).map_err(|reason| (4, reason))?;
    let generation = decode_hex(&transfer.history_generation).map_err(|reason| (4, reason))?;
    let scope = transfer.scope.into();
    let mut writer = RelWriter::create_current_path(staged_file, scope, owner)
        .map_err(|_| (7, "could not initialize a current owner"))?;
    let mut cuts = vec![proof(&writer).map_err(|reason| (6, reason))?];
    if cuts[0].1.history_generation != generation {
        return Err((4, "unsupported synthetic owner history generation"));
    }
    for event in transfer.events {
        if event.write(&mut writer).map_err(|reason| (4, reason))? {
            cuts.push(proof(&writer).map_err(|reason| (6, reason))?);
        }
    }
    let written = writer
        .read_session()
        .archive_publication_bindings()
        .to_vec();
    if written.is_empty() {
        return Err((4, "synthetic transfer has no Archive publications"));
    }
    drop(writer);

    let reader = RelSession::open_path_current(staged_file)
        .map_err(|_| (6, "candidate failed normal current-only reopen"))?;
    if reader.owner_id() != owner
        || reader.scope() != scope
        || cuts.last().map(|(cut, _)| cut) != Some(&reader.current_historical_cut())
        || reader.archive_publication_bindings() != written
    {
        return Err((
            6,
            "candidate Archive owner or publication bindings changed on reopen",
        ));
    }
    let mut history = Vec::with_capacity(cuts.len());
    for (cut, proof) in &cuts {
        if reader.historical_proof(owner, cut).as_ref() != Ok(proof) {
            return Err((6, "candidate Archive publication history changed on reopen"));
        }
        history.push(json!({
            "archive_version": cut.archive_version.to_string(),
            "retained_records": proof.retained_records.to_string(),
            "proof_sha256": hex::encode(proof.digest)
        }));
    }
    Ok(json!({
        "schema_id": "reliquary-d01-synthetic-archive-bindings-receipt-v1",
        "scope": "synthetic_versioned_archive_only",
        "owner": hex::encode(owner),
        "history_generation": hex::encode(generation),
        "archive_history": history,
        "archive_publication_bindings": written.iter().map(binding_value).collect::<Vec<_>>()
    }))
}

fn binding_value(binding: &ArchivePublicationBinding) -> Value {
    json!({
        "archive_version": binding.archive_version.to_string(),
        "global_version": binding.global_version.to_string(),
        "transaction_time_ns": binding.transaction_time_ns.map(|time| time.to_string()),
        "bound_reservation": binding.bound_reservation,
        "first_applying_archive_version": binding.first_applying_archive_version.to_string(),
        "first_applying_global_version": binding.first_applying_global_version.to_string(),
    })
}

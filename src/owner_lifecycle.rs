//! Validated owner lifecycle boundary and read-only historical source capability.

#[path = "owner_lifecycle/history_source.rs"]
mod history_source;
pub(crate) use history_source::{
    HistoryFrame, HistoryObject, HistoryReadAccess, HistoryReadFailure, HistorySource,
};

#[path = "owner_lifecycle/vocabulary.rs"]
mod vocabulary;
pub use vocabulary::{
    HistoricalPayload, LifecycleError, LifecycleState, LifecycleTarget, PayloadUnavailable,
    TombstoneCoordinates, TombstoneOutcome, TombstonePolicy,
};

#[path = "owner_lifecycle/error.rs"]
mod error;
pub use error::OwnerError;

#[path = "owner_lifecycle/vector_publication.rs"]
pub(crate) mod vector_publication;

#[path = "owner_lifecycle/memory_frames.rs"]
pub(crate) mod memory_frames;

#[path = "owner_lifecycle/insomnia_application.rs"]
pub(crate) mod insomnia_application;

#[path = "owner_lifecycle/insomnia_replay.rs"]
pub(crate) mod insomnia_replay;

#[path = "owner_lifecycle/insomnia_import_replay.rs"]
pub(crate) mod insomnia_import_replay;

#[path = "owner_lifecycle/open_route.rs"]
pub(crate) mod open_route;

#[path = "owner_lifecycle/reliquary_replay.rs"]
mod reliquary_replay;
pub(crate) use reliquary_replay::{ReliquaryReconstruction, ReliquaryReplay};

#[path = "owner_lifecycle/phylactery_error.rs"]
mod phylactery_error;
pub use phylactery_error::OwnerPhylacteryError;

#[path = "owner_lifecycle/phy_profile/mod.rs"]
mod phy_profile;
pub(crate) use phy_profile::PhylacteryProfileStore;
pub use phy_profile::{MAX_PHYLACTERY_PROFILE_NAME_BYTES, PhylacteryProfile};

#[path = "owner_lifecycle/read_validation.rs"]
mod read_validation;
pub(crate) use read_validation::{
    validate_current_graph_node, validate_current_relationship_references, validate_dream_pairs,
    validate_entity_resolution_references, validate_phylactery_draft_provenance,
    validate_phylactery_global_versions, validate_phylactery_provenance,
    validate_principal_graph_kind, validate_rebuilt_graph, validate_relationship_entities,
};

#[path = "owner_lifecycle/phylactery.rs"]
mod phylactery;
pub(crate) use phylactery::{
    PhylacteryOwner, initialize_phylactery, open_phylactery_from_container,
};

#[path = "owner_lifecycle/global_versions.rs"]
pub(crate) mod global_versions;
#[path = "owner_lifecycle/memory_provenance.rs"]
pub(crate) mod memory_provenance;

#[path = "owner_lifecycle/reclamation_source.rs"]
mod reclamation_source;
#[path = "owner_lifecycle/relocation.rs"]
pub(crate) mod relocation;
pub(crate) use reclamation_source::{
    PhylacteryReclamationCopy, PhylacteryReclamationSource, ReclamationCopyStats,
    ReliquaryReclamationCopy, ReliquaryReclamationSource,
};

#[path = "owner_lifecycle/publication_births.rs"]
mod publication_births;
pub(crate) use publication_births::{
    AcceptedExternalBirthCohort, AcceptedExternalBirthMember, AcceptedInsomniaBirthCohort,
    AcceptedInsomniaBirthMember, DreamPopulationPrefix, direct_birth_ids,
    validate_external_birth_cohort, validate_insomnia_birth_cohort,
};

#[path = "owner_lifecycle/external_index_backing.rs"]
mod external_index_backing;
pub(crate) use external_index_backing::validate_external_index_backing;
#[path = "owner_lifecycle/import_receipts.rs"]
mod import_receipts;
pub(crate) use import_receipts::validate_import_receipts;
#[path = "owner_lifecycle/dream_replay.rs"]
pub(crate) mod dream_replay;

#[path = "owner_lifecycle/reliquary.rs"]
mod reliquary;
pub(crate) use reliquary::{ReliquaryOwner, initialize_reliquary};
/// Validated native owner and its already reconstructed, non-durable due projection.
pub(crate) type ReconstructedReliquary = (ReliquaryOwner, crate::FreshnessDueDispatcher);

#[path = "owner_lifecycle/freshness_replay.rs"]
mod freshness_replay;
pub(crate) use freshness_replay::rebuild_freshness_from_replay;

#[path = "owner_lifecycle/reliquary_relations.rs"]
mod reliquary_relations;
pub(crate) use reliquary_relations::{ReliquaryRelations, finish_reliquary_relations};
#[path = "owner_lifecycle/reliquary_vectors.rs"]
mod reliquary_vectors;
pub(crate) use reliquary_vectors::{ReliquaryVectors, finish_reliquary_vectors};

#[path = "owner_lifecycle/reliquary_archive.rs"]
mod reliquary_archive;
pub(crate) use reliquary_archive::finish_reliquary_archive;

#[path = "owner_lifecycle/reliquary_graph.rs"]
mod reliquary_graph;
pub(crate) use reliquary_graph::finish_reliquary_graph;

#[path = "owner_lifecycle/reliquary_accepted_replay.rs"]
mod reliquary_accepted_replay;
pub(crate) use reliquary_accepted_replay::{
    ReliquaryAcceptedReplay, finish_reliquary_accepted_replay,
};

#[path = "owner_lifecycle/import_session.rs"]
mod import_session;
pub(crate) use import_session::ImportSession;

#[path = "owner_lifecycle/import_completion_access.rs"]
mod import_completion_access;
pub(crate) use import_completion_access::ImportCompletionAccess;

#[path = "owner_lifecycle/import_epoch_transport.rs"]
pub(crate) mod import_epoch_transport;

#[path = "owner_lifecycle/graph_ordered_replay.rs"]
pub(crate) mod graph_ordered_replay;

#[path = "owner_lifecycle/entity_resolution_admission.rs"]
pub(crate) mod entity_resolution_admission;

#[path = "owner_lifecycle/entity_publication_admission.rs"]
pub(crate) mod entity_publication_admission;

#[path = "owner_lifecycle/perception_publication_resolution.rs"]
pub(crate) mod perception_publication_resolution;

#[path = "owner_lifecycle/archive_ordered_replay.rs"]
pub(crate) mod archive_ordered_replay;
#[path = "owner_lifecycle/archive_replay_error.rs"]
pub(crate) mod archive_replay_error;

#[path = "owner_lifecycle/physical_copy.rs"]
pub(crate) mod physical_copy;

#[path = "owner_lifecycle/freshness_reconciliation.rs"]
pub(crate) mod freshness_reconciliation;

#[path = "owner_lifecycle/native_import.rs"]
pub(crate) mod native_import;

#[path = "owner_lifecycle/dream_begin_transport.rs"]
pub(crate) mod dream_begin_transport;

#[path = "owner_lifecycle/grouped_import_replay.rs"]
pub(crate) mod grouped_import_replay;

#[path = "owner_lifecycle/semantic_source.rs"]
pub(crate) mod semantic_source;

#[path = "owner_lifecycle/import_birth_facts.rs"]
pub(crate) mod import_birth_facts;

#[path = "owner_lifecycle/historical_settlement.rs"]
pub(crate) mod historical_settlement;

#[path = "owner_lifecycle/historical_freshness_read.rs"]
pub(crate) mod historical_freshness_read;

#[path = "owner_lifecycle/physical_fingerprint.rs"]
pub(crate) mod physical_fingerprint;

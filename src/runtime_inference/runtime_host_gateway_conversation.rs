use super::owner_operations::{ActorClass, AdmissionIdentity, Attribution, OwnerOperationJournal};
#[cfg(test)]
use super::{
    GatewayHostAdapter, HandoffReceipt, HostCapabilityContract, HostConnection,
    HostQueueGuarantees, HostReconcileStatus,
};
use super::{
    GatewayRuntime, HostCancelOutcome, HostOperation, HostOperationStatus, InstanceRuntime,
    OwnerAccessToken, OwnerAdmission, OwnerGrant, ReliquaryRuntimeHostError, RuntimeCapability,
    StageAuthority, operation,
};
use crate::multiplexing_journal::{
    ArchiveReceipt, AttachmentIdentity, ConversationId, ConversationSubmissionId,
    DurableGeneration, DurableGenerationPhase, DurableHostOperationPhase, EnqueueOutcome,
    FinalizeCapture, JournalError, PostCommitReceipt, RelJournal, RelOwnerId, Submission,
    SubmissionId, SubmissionState, SubmissionStatus, TransitionOutcome,
};
use crate::{
    Branch, InteractionAttachment, InteractionRole, InteractionStreamRecord,
    InteractionStreamStatus, InteractionTurn, ResolvedTurn, StoredFile,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayConversationAttachment {
    pub attachment_id: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayConversationSubmission {
    pub conversation_id: String,
    pub submission_id: String,
    pub expected_conversation_revision: u64,
    pub expected_parent_node_id: Option<String>,
    pub branch_id: String,
    pub content: String,
    pub attachments: Vec<GatewayConversationAttachment>,
    pub client_timestamp_ns: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewaySubmissionReceipt {
    pub id: ConversationSubmissionId,
    pub status: SubmissionStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationViewerToken {
    owner_id: String,
    conversation_id: String,
    instance_id: super::InstanceId,
    nonce: Uuid,
    mount_epoch: u64,
}
impl ConversationViewerToken {
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }
    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }
}

#[path = "runtime_host_gateway_conversation_continuation.rs"]
mod continuation;
#[path = "runtime_host_gateway_conversation_generation.rs"]
mod generation;
#[path = "runtime_host_gateway_conversation_observation.rs"]
mod observation;
#[path = "runtime_host_gateway_conversation_recovery.rs"]
mod recovery;
use continuation::ConversationCoordinator;
use generation::*;
use recovery::archive_receipt_if_present;
#[derive(Default)]
struct ViewerState {
    viewers: HashMap<(String, String), HashMap<Uuid, super::InstanceId>>,
}

/// One REL-scoped journal writer plus continuations keyed by (REL, conversation).
pub(super) struct ConversationRegistry {
    state_dir: PathBuf,
    journals: RwLock<BTreeMap<String, Arc<Mutex<RelJournal>>>>,
    owner_operations: RwLock<BTreeMap<String, Arc<Mutex<OwnerOperationJournal>>>>,
    coordinators: Mutex<HashMap<(String, String), Arc<ConversationCoordinator>>>,
    viewers: Mutex<ViewerState>,
}
impl ConversationRegistry {
    pub(super) fn new(state_dir: PathBuf) -> Self {
        Self {
            state_dir,
            journals: RwLock::new(BTreeMap::new()),
            owner_operations: RwLock::new(BTreeMap::new()),
            coordinators: Mutex::new(HashMap::new()),
            viewers: Mutex::new(ViewerState::default()),
        }
    }
    pub(super) fn mount_rel(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        let rel_id = RelOwnerId::parse_canonical(owner_id).map_err(operation)?;
        let operations =
            OwnerOperationJournal::open(&self.state_dir, &rel_id.as_uuid().to_string())
                .map_err(operation)?;
        let journal = RelJournal::open(&self.state_dir, rel_id).map_err(operation)?;
        let mut journals = self
            .journals
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if journals.contains_key(owner_id) {
            return Err(operation(format!(
                "journal for {owner_id} is already mounted"
            )));
        }
        self.owner_operations
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .insert(owner_id.to_owned(), Arc::new(Mutex::new(operations)));
        journals.insert(owner_id.to_owned(), Arc::new(Mutex::new(journal)));
        Ok(())
    }
    pub(super) fn unmount_rel(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        self.journals
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .remove(owner_id)
            .ok_or_else(|| operation(format!("journal for {owner_id} is not mounted")))?;
        self.owner_operations
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .remove(owner_id);
        self.coordinators
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .retain(|(rel, _), _| rel != owner_id);
        self.viewers
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .viewers
            .retain(|(rel, _), _| rel != owner_id);
        Ok(())
    }
    pub(super) fn shutdown(&self) -> Result<(), ReliquaryRuntimeHostError> {
        self.owner_operations
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .clear();
        self.viewers
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .viewers
            .clear();
        self.coordinators
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .clear();
        self.journals
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .clear();
        Ok(())
    }
    pub(super) fn remove_instance_owner_viewers(
        &self,
        id: super::InstanceId,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut viewers = self
            .viewers
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        for ((rel_id, _), registrations) in viewers.viewers.iter_mut() {
            if rel_id == owner_id {
                registrations.retain(|_, instance| *instance != id);
            }
        }
        viewers
            .viewers
            .retain(|_, registrations| !registrations.is_empty());
        Ok(())
    }
    pub(super) fn detach_instance_viewers(
        &self,
        id: super::InstanceId,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut viewers = self
            .viewers
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        for registrations in viewers.viewers.values_mut() {
            registrations.retain(|_, instance| *instance != id);
        }
        viewers
            .viewers
            .retain(|_, registrations| !registrations.is_empty());
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn journal_for_test(
        &self,
        owner_id: &str,
    ) -> Result<Arc<Mutex<RelJournal>>, ReliquaryRuntimeHostError> {
        self.journal(owner_id)
    }

    pub(super) fn journal(
        &self,
        owner_id: &str,
    ) -> Result<Arc<Mutex<RelJournal>>, ReliquaryRuntimeHostError> {
        self.journals
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
            .cloned()
            .ok_or_else(|| operation(format!("journal for {owner_id} is not mounted")))
    }
    /// Persist withdrawal for original admissions, including currently offline
    /// RELs. The caller holds the lifecycle and publication-policy write gates.
    pub(super) fn revoke_personal_authority(
        &self,
        instance_id: super::InstanceId,
        revoked_rels: &BTreeSet<String>,
        revoked_phy: Option<&str>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mounted = self
            .owner_operations
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .clone();
        let mut owners = revoked_rels.clone();
        if revoked_phy.is_some() {
            owners.extend(mounted.keys().cloned());
            let directory = self.state_dir.join("owner-operations");
            if directory.exists() {
                for entry in std::fs::read_dir(directory).map_err(operation)? {
                    let path = entry.map_err(operation)?.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("roj") {
                        continue;
                    }
                    let id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| operation("invalid owner-operation filename"))?;
                    let rel = uuid::Uuid::parse_str(id).map_err(operation)?;
                    if rel.is_nil() || rel.to_string() != id {
                        return Err(operation("noncanonical owner-operation filename"));
                    }
                    owners.insert(format!("rel-{id}"));
                }
            }
        }
        for owner in owners {
            let revoke_rel = revoked_rels.contains(&owner);
            if let Some(journal) = mounted.get(&owner) {
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .revoke_personal_authority(&instance_id.to_string(), revoke_rel, revoked_phy)
                    .map_err(operation)?;
            } else {
                let rel = RelOwnerId::parse_canonical(&owner).map_err(operation)?;
                let uuid = rel.as_uuid().to_string();
                if !self
                    .state_dir
                    .join("owner-operations")
                    .join(format!("{uuid}.roj"))
                    .exists()
                {
                    continue; // Lost attribution has no personal routing authority.
                }
                OwnerOperationJournal::open(&self.state_dir, &uuid)
                    .map_err(operation)?
                    .revoke_personal_authority(&instance_id.to_string(), revoke_rel, revoked_phy)
                    .map_err(operation)?;
            }
        }
        Ok(())
    }

    pub(super) fn owner_journal(
        &self,
        owner_id: &str,
    ) -> Result<Arc<Mutex<OwnerOperationJournal>>, ReliquaryRuntimeHostError> {
        self.owner_operations
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
            .cloned()
            .ok_or_else(|| {
                operation(format!(
                    "owner-operation journal for {owner_id} is not mounted"
                ))
            })
    }

    fn coordinator(
        &self,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<Arc<ConversationCoordinator>, ReliquaryRuntimeHostError> {
        let mut coordinators = self
            .coordinators
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        Ok(Arc::clone(
            coordinators
                .entry((owner_id.to_owned(), conversation_id.to_owned()))
                .or_insert_with(|| Arc::new(ConversationCoordinator::new())),
        ))
    }
}

/// Mounted-owner fence for an already durable operation.
/// The journal owns principal and scope, independently of attachments.
struct OperationAuthority {
    owner_id: String,
    mount_epoch: u64,
    fence_generation: u64,
}
impl OperationAuthority {
    fn capture(
        gateway: &GatewayRuntime,
        owner_id: &str,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        let rels = gateway
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let rel = rels
            .get(owner_id)
            .ok_or_else(|| operation("conversation REL is not mounted"))?;
        let (admission, fence_generation) = rel.fence.snapshot()?;
        if admission != OwnerAdmission::Open {
            return Err(operation("conversation REL is quiesced"));
        }
        Ok(Self {
            owner_id: owner_id.to_owned(),
            mount_epoch: rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst),
            fence_generation,
        })
    }
}

impl GatewayRuntime {
    /// Read canonical Archive policy through the requesting instance's REL grant.
    pub fn conversation_policy(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<(crate::ConversationPolicy, u64), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let conversation_id = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        self.with_rel(&token, |runtime| {
            Ok(runtime.cva().conversation_policy(conversation_id.as_str()))
        })
    }

    /// ManageRel is required; neither viewing nor submitting implies management.
    /// The owner runtime lock also serializes policy publication with FinalizeWon.
    pub fn set_conversation_policy(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        policy: crate::ConversationPolicy,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let conversation_id = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        with_conversation_admission(self, &token, || {
            self.with_rel(&token, |runtime| {
                let changed = runtime
                    .cva
                    .set_conversation_policy(conversation_id.as_str(), policy)
                    .map_err(operation)?;
                // Also synchronize an idempotent retry after a prior failed sync.
                runtime.cva.sync().map_err(operation)?;
                Ok(changed)
            })
        })
    }

    /// Admission is acknowledged only after the REL journal has durably accepted it.
    pub fn submit_conversation(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        request: GatewayConversationSubmission,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::SubmitConversation)?;
        let conversation_id =
            ConversationId::new(request.conversation_id.clone()).map_err(operation)?;
        let submission_id = SubmissionId::new(request.submission_id.clone()).map_err(operation)?;
        if request.branch_id.trim().is_empty() {
            return Err(operation("branch ID must not be empty"));
        }
        if request.content.trim().is_empty() && request.attachments.is_empty() {
            return Err(operation(
                "conversation submission requires text or an attachment",
            ));
        }
        let id = ConversationSubmissionId {
            conversation_id: conversation_id.clone(),
            submission_id: submission_id.clone(),
        };
        let journal = self.conversation.journal(owner_id)?;
        let already_admitted = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&id)
            .is_some();
        // Do not reinterpret an accepted request against a later branch head.
        // The journal still verifies its exact immutable request on every retry.
        // Release the journal before acquiring the semantic owner runtime.
        self.with_rel(&token, |runtime| {
            if already_admitted {
                return Ok(());
            }
            if let Some(branch) = runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .find(|branch| {
                    branch.conversation_id == request.conversation_id
                        && branch.id == request.branch_id
                })
            {
                if request.expected_parent_node_id.as_deref() != Some(branch.leaf_node_id.as_str())
                {
                    return Err(operation(
                        "expected parent does not identify the selected branch head",
                    ));
                }
            } else {
                let leaves = runtime
                    .conversation_summaries()
                    .into_iter()
                    .find(|summary| summary.conversation_id == request.conversation_id)
                    .map(|summary| summary.leaf_node_ids)
                    .unwrap_or_default();
                if request
                    .expected_parent_node_id
                    .as_ref()
                    .is_some_and(|parent| !leaves.contains(parent))
                    || (request.expected_parent_node_id.is_none() && !leaves.is_empty())
                {
                    return Err(operation(
                        "expected parent must be the current conversation leaf",
                    ));
                }
            }
            Ok(())
        })?;
        RelJournal::validate_staged_payload_sizes(
            request
                .attachments
                .iter()
                .map(|attachment| attachment.bytes.len()),
        )
        .map_err(operation)?;
        let identities = request
            .attachments
            .iter()
            .map(|attachment| AttachmentIdentity {
                attachment_id: attachment.attachment_id.clone(),
                filename: attachment.filename.clone(),
                mime_type: attachment.mime_type.clone(),
                content_hash: Sha256::digest(&attachment.bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            })
            .collect::<Vec<_>>();
        let payload_hash =
            Submission::canonical_payload_hash(&request.content, &identities).map_err(operation)?;
        let scope_fingerprint = fingerprint(&json!({
            "rel_id": owner_id, "conversation_id": request.conversation_id, "branch_id": request.branch_id,
        }))?;
        let selected_branch_id = request.branch_id.clone();
        let submission = Submission {
            id: id.clone(),
            principal_id: instance.principal_id().to_owned(),
            scope_fingerprint,
            payload_hash,
            request_metadata: json!({
                "branch_id": request.branch_id, "expected_parent_node_id": request.expected_parent_node_id,
                "admission": {
                    "owner_id": owner_id, "principal_id": instance.principal_id(),
                    "grant": "SubmitConversation", "policy_revision": 1,
                },
            }),
            content: request.content,
            attachments: identities.clone(),
            client_timestamp_ns: request.client_timestamp_ns,
        };
        let receive_ns = now_ns();
        let tie_key = *Uuid::new_v4().as_bytes();
        let expected_revision = request.expected_conversation_revision;
        let operations = self.conversation.owner_journal(owner_id)?;
        // A PHY binding names a destination; only a current UseOwnPhy token
        // supplies permission. Both admission leases cover the durable capture.
        let personal_token = instance
            .attached_phy_owner_id()
            .and_then(|phy| self.authorize(instance, phy, OwnerGrant::UseOwnPhy).ok());
        let attribution = Attribution {
            generation_id: conversation_host_operation_id(owner_id, &id),
            conversation_id: id.conversation_id.as_str().to_owned(),
            submission_id: id.submission_id.as_str().to_owned(),
            actor_class: ActorClass::User,
            actor_principal: Some(instance.principal_id().to_owned()),
            generation_initiator: Some(instance.principal_id().to_owned()),
            initiating_generation_id: None,
            admission: AdmissionIdentity {
                instance_id: instance.instance_id().to_string(),
                principal_id: Some(instance.principal_id().to_owned()),
                phy_owner_id: instance.attached_phy_owner_id().map(str::to_owned),
                personal: personal_token.as_ref().map(|token| {
                    super::owner_operations::PersonalAdmission {
                        mount_epoch: token.mount_epoch,
                        grant_generation: token.owner_grant_generation,
                    }
                }),
                mount_epoch: token.mount_epoch,
                grant_generation: token.owner_grant_generation,
            },
        };
        let submit = || {
            let mut journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            compact_if_needed(&mut journal)?;
            let idempotent = journal
                .preflight_enqueue_frame(&submission, receive_ns, expected_revision, tie_key)
                .map_err(operation)?;
            if !idempotent {
                let staged = journal
                    .stage_attachments(request.attachments.iter().map(|attachment| {
                        (
                            attachment.attachment_id.as_str(),
                            attachment.filename.as_str(),
                            attachment.mime_type.as_deref(),
                            attachment.bytes.as_slice(),
                        )
                    }))
                    .map_err(operation)?;
                if staged != identities {
                    return Err(operation(
                        "staged attachment identity changed during admission",
                    ));
                }
            }
            // Sync source authority before accepting work that depends on it.
            // Retries from a second attachment retain the original admission.
            {
                let mut operations = operations
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                match operations.attribution(&attribution.generation_id) {
                    Some(existing)
                        if existing.actor_principal == attribution.actor_principal
                            && existing.generation_initiator
                                == attribution.generation_initiator
                            && existing.admission.phy_owner_id
                                == attribution.admission.phy_owner_id
                            && existing.conversation_id == attribution.conversation_id
                            && existing.submission_id == attribution.submission_id
                            && existing.actor_class == ActorClass::User => {}
                    Some(_) => return Err(operation("immutable source attribution conflicts")),
                    None if idempotent => {} // Legacy accepted work remains Unknown.
                    None => {
                        operations
                            .append_attribution(attribution.clone())
                            .map_err(operation)?;
                    }
                }
            }
            let outcome = match journal.enqueue_with_tie_key(
                submission,
                receive_ns,
                expected_revision,
                tie_key,
            ) {
                Ok(outcome) => outcome,
                Err(JournalError::JournalFull) if !idempotent => {
                    journal
                        .collect_unreferenced_attachments()
                        .map_err(operation)?;
                    return Err(operation(JournalError::JournalFull));
                }
                Err(error) => return Err(operation(error)),
            };
            match outcome {
                EnqueueOutcome::Durable(status) | EnqueueOutcome::Idempotent(status) => Ok(status),
            }
        };
        let status = with_conversation_admission(self, &token, || match personal_token.as_ref() {
            Some(personal) => with_admission(self, personal, submit),
            None => submit(),
        })?;
        // Durable submit is the explicit writer-admission point. Publish the
        // instance-local selector only after the REL journal has acknowledged it.
        // Selection is advisory instance state: the durable receipt remains
        // authoritative if this attachment is revoked immediately after enqueue.
        let _ = self.select_rel(instance, owner_id);
        let _ = self.set_instance_managed_state(
            instance,
            owner_id,
            super::super::instances::ManagedConversationState {
                conversation_id: Some(id.conversation_id.as_str().to_owned()),
                branch_id: Some(selected_branch_id),
                selected_leaf_node_id: None,
                reopened_session_pending: false,
            },
        );
        let _ = self
            .conversation
            .coordinator(owner_id, id.conversation_id.as_str())?;
        Ok(GatewaySubmissionReceipt { id, status })
    }

    /// Explicitly admit this attached instance to the gateway-owned writable
    /// continuation. The returned value is only a snapshot; the mutable session
    /// stays inside the shared InteractionRuntime. Repeated admission reuses the
    /// same owner runtime; the selected Archive branch supplies the cursor.
    /// It never creates a per-viewer continuation.
    pub fn admit_conversation_writer(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<crate::InteractionSession, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::SubmitConversation)?;
        let conversation_id = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        if branch_id.trim().is_empty() {
            return Err(operation("branch ID must not be empty"));
        }
        let session = with_conversation_admission(self, &token, || {
            self.with_rel(&token, |runtime| {
                let branch = runtime
                    .cva()
                    .archive()
                    .branches()
                    .into_iter()
                    .find(|branch| {
                        branch.conversation_id == conversation_id.as_str() && branch.id == branch_id
                    })
                    .ok_or_else(|| operation("writer admission branch does not exist"))?;
                ensure_session_at(
                    runtime,
                    conversation_id.as_str(),
                    Some(&branch.leaf_node_id),
                )?;
                runtime
                    .session(conversation_id.as_str())
                    .ok_or_else(|| operation("shared continuation session disappeared"))
            })
        })?;
        let _ = self.select_rel(instance, owner_id);
        let _ = self.set_instance_managed_state(
            instance,
            owner_id,
            super::super::instances::ManagedConversationState {
                conversation_id: Some(conversation_id.as_str().to_owned()),
                branch_id: Some(branch_id.to_owned()),
                selected_leaf_node_id: None,
                reopened_session_pending: false,
            },
        );
        Ok(session)
    }

    /// Select a managed conversation for this instance without opening a
    /// session. Viewer selection never creates or mutates the shared continuation.
    pub fn select_managed_conversation(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<super::super::instances::ManagedConversationState, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let conversation_id = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        if branch_id.trim().is_empty() {
            return Err(operation("branch ID must not be empty"));
        }
        self.with_rel(&token, |runtime| {
            if !runtime.cva().archive().branches().iter().any(|branch| {
                branch.conversation_id == conversation_id.as_str() && branch.id == branch_id
            }) {
                return Err(operation(
                    "selected branch does not exist in this conversation",
                ));
            }
            Ok(())
        })?;
        self.select_rel(instance, owner_id)?;
        let managed = super::super::instances::ManagedConversationState {
            conversation_id: Some(conversation_id.as_str().to_owned()),
            branch_id: Some(branch_id.to_owned()),
            selected_leaf_node_id: None,
            reopened_session_pending: true,
        };
        self.set_instance_managed_state(instance, owner_id, managed.clone())?;
        Ok(managed)
    }

    pub fn attach_conversation_viewer(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<ConversationViewerToken, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let conversation_id = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        self.with_rel(&token, |_| Ok(()))?;
        let nonce = Uuid::new_v4();
        with_admission(self, &token, || {
            self.conversation
                .viewers
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .viewers
                .entry((owner_id.to_owned(), conversation_id.as_str().to_owned()))
                .or_default()
                .insert(nonce, instance.instance_id());
            Ok(())
        })?;
        Ok(ConversationViewerToken {
            owner_id: owner_id.to_owned(),
            conversation_id: conversation_id.as_str().to_owned(),
            instance_id: instance.instance_id(),
            nonce,
            mount_epoch: token.mount_epoch,
        })
    }

    pub fn detach_conversation_viewer(
        &self,
        token: ConversationViewerToken,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let rel = rels
            .get(&token.owner_id)
            .ok_or_else(|| operation("viewer REL is not mounted"))?;
        if rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst) != token.mount_epoch {
            return Err(operation("viewer token belongs to a previous REL mount"));
        }
        let mut viewers = self
            .conversation
            .viewers
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let registration = viewers
            .viewers
            .get_mut(&(token.owner_id.clone(), token.conversation_id.clone()))
            .ok_or_else(|| operation("viewer attachment is no longer active"))?;
        if registration.remove(&token.nonce) != Some(token.instance_id) {
            return Err(operation("viewer token is invalid"));
        }
        if registration.is_empty() {
            viewers
                .viewers
                .remove(&(token.owner_id, token.conversation_id));
        }
        Ok(())
    }

    /// Dispatch the oldest eligible durable entry. None means idle or all branches busy.
    pub fn process_next_conversation_submission(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<Option<GatewaySubmissionReceipt>, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::SubmitConversation)?;
        with_admission(self, &token, || Ok(()))?;
        let receipt =
            self.process_next_accepted_conversation_submission(owner_id, conversation_id)?;
        with_admission(self, &token, || Ok(receipt))
    }

    /// Accepted work remains dispatchable after its submitting instance detaches.
    pub(super) fn process_next_accepted_conversation_submission(
        &self,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<Option<GatewaySubmissionReceipt>, ReliquaryRuntimeHostError> {
        let authority = OperationAuthority::capture(self, owner_id)?;
        let conversation = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        let journal = self.conversation.journal(owner_id)?;
        let coordinator = self.conversation.coordinator(owner_id, conversation_id)?;
        let Some((key, guard)) = coordinator.acquire_next(&journal, &conversation)? else {
            return Ok(None);
        };
        self.process_reserved_conversation_submission(owner_id, authority, journal, key, guard)
            .map(Some)
    }

    /// Reconcile only work that already owns a durable continuation. This cannot
    /// select a fresh Pending submission or bypass chronological admission.
    pub fn resume_conversation_submission(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::SubmitConversation)?;
        with_admission(self, &token, || Ok(()))?;
        let authority = OperationAuthority::capture(self, owner_id)?;
        let key = ConversationSubmissionId {
            conversation_id: ConversationId::new(conversation_id.to_owned()).map_err(operation)?,
            submission_id: SubmissionId::new(submission_id.to_owned()).map_err(operation)?,
        };
        let journal = self.conversation.journal(owner_id)?;
        let branch_id =
            {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let status = journal
                    .status(&key)
                    .ok_or_else(|| operation("submission not found"))?;
                if status.state == SubmissionState::Cancelled
                    || (status.state == SubmissionState::Committed
                        && status.post_commit_receipt.is_some())
                {
                    drop(journal);
                    return with_admission(self, &token, || {
                        Ok(GatewaySubmissionReceipt { id: key, status })
                    });
                }
                if status.state == SubmissionState::Pending
                    && journal
                        .host_operation(&key, RuntimeCapability::ConversationExecution)
                        .is_none()
                {
                    return Err(operation(
                        "fresh Pending work must use coordinator scheduling",
                    ));
                }
                match status.finalize_capture {
                    Some(capture) => capture.branch_id,
                    None => metadata_branch(journal.submission(&key).ok_or_else(|| {
                        operation("unfinished submission payload is not retained")
                    })?),
                }
            };
        let coordinator = self.conversation.coordinator(owner_id, conversation_id)?;
        let guard = coordinator.acquire(&journal, &key, &branch_id)?;
        let receipt = self
            .process_reserved_conversation_submission(owner_id, authority, journal, key, guard)?;
        with_admission(self, &token, || Ok(receipt))
    }

    #[cfg(test)]
    fn process_exact_conversation_submission_for_test(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::SubmitConversation)?;
        with_admission(self, &token, || Ok(()))?;
        let receipt = self.process_accepted_conversation_submission(
            owner_id,
            conversation_id,
            submission_id,
        )?;
        with_admission(self, &token, || Ok(receipt))
    }

    /// Exact-entry fault injection is deliberately absent from production builds.
    #[cfg(test)]
    pub(super) fn process_accepted_conversation_submission(
        &self,
        owner_id: &str,
        conversation_id: &str,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let authority = OperationAuthority::capture(self, owner_id)?;
        let key = ConversationSubmissionId {
            conversation_id: ConversationId::new(conversation_id.to_owned()).map_err(operation)?,
            submission_id: SubmissionId::new(submission_id.to_owned()).map_err(operation)?,
        };
        let journal = self.conversation.journal(owner_id)?;
        let branch_id = {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let status = journal
                .status(&key)
                .ok_or_else(|| operation("submission not found"))?;
            if status.state == SubmissionState::Cancelled
                || (status.state == SubmissionState::Committed
                    && status.post_commit_receipt.is_some())
            {
                return Ok(GatewaySubmissionReceipt { id: key, status });
            }
            status
                .finalize_capture
                .map(|capture| capture.branch_id)
                .unwrap_or_else(|| {
                    metadata_branch(
                        journal
                            .submission(&key)
                            .expect("retained unfinished payload"),
                    )
                })
        };
        let coordinator = self.conversation.coordinator(owner_id, conversation_id)?;
        let guard = coordinator.acquire_exact_for_test(&journal, &key, &branch_id)?;
        self.process_reserved_conversation_submission(owner_id, authority, journal, key, guard)
    }

    fn process_reserved_conversation_submission(
        &self,
        owner_id: &str,
        operation_authority: OperationAuthority,
        journal: Arc<Mutex<RelJournal>>,
        key: ConversationSubmissionId,
        _processing: continuation::ProcessingGuard,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let mut status = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        if status.state == SubmissionState::Cancelled
            || (status.state == SubmissionState::Committed && status.post_commit_receipt.is_some())
        {
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }
        let submission = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .submission(&key)
            .cloned()
            .ok_or_else(|| operation("submission payload is not retained"))?;
        validate_accepted_submission(owner_id, &key, &submission)?;
        // Another attempt may have completed before this branch lease was acquired.
        status = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        if status.state == SubmissionState::Cancelled
            || (status.state == SubmissionState::Committed && status.post_commit_receipt.is_some())
        {
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }
        if status.state == SubmissionState::Pending {
            if let Some(host_status) = drive_host_conversation_stage(
                self,
                &operation_authority,
                owner_id,
                &journal,
                &key,
                &submission,
            )? {
                return Ok(GatewaySubmissionReceipt {
                    id: key,
                    status: host_status,
                });
            }
        }
        if status.state == SubmissionState::Pending {
            status = with_operation(self, &operation_authority, |runtime| {
                let (policy, policy_revision) = runtime
                    .cva()
                    .conversation_policy(key.conversation_id.as_str());
                let target = finalization::finalization_target(
                    runtime,
                    key.conversation_id.as_str(),
                    &submission,
                    policy,
                )?;
                ensure_session_at(
                    runtime,
                    key.conversation_id.as_str(),
                    target.parent_node_id.as_deref(),
                )?;
                // Runtime -> journal is the only nested order here. Policy management
                // holds this same runtime lock, so publication cannot race capture.
                // A prior failed management sync cannot leak an undurable policy.
                runtime.cva.sync().map_err(operation)?;
                let capture = FinalizeCapture {
                    branch_id: target.branch_id,
                    branch_label: target.branch_label,
                    parent_node_id: target.parent_node_id,
                    committed_timestamp_ns: now_ns(),
                    verified_principal_id: submission.principal_id.clone(),
                    policy_revision,
                };
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                compact_if_needed(&mut journal)?;
                let revision = journal.conversation_revision(&key.conversation_id);
                match journal
                    .claim_finalize(&key, revision, capture)
                    .map_err(operation)?
                {
                    TransitionOutcome::Durable(status) | TransitionOutcome::Idempotent(status) => {
                        Ok(status)
                    }
                }
            })?;
        }
        if status.state == SubmissionState::Finalizing {
            let capture = status
                .finalize_capture
                .clone()
                .ok_or_else(|| operation("finalizing entry lacks its captured turn"))?;
            let attachment_bytes = {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                submission
                    .attachments
                    .iter()
                    .map(|identity| {
                        journal
                            .read_attachment(identity)
                            .map(|bytes| (identity.clone(), bytes))
                            .map_err(operation)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            let archive_receipt = with_operation(self, &operation_authority, |runtime| {
                archive_receipt_or_commit(runtime, &key, &submission, &capture, &attachment_bytes)
            })?;
            status = with_accepted_work(self, &operation_authority, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                compact_if_needed(&mut journal)?;
                match journal
                    .record_archive_commit(&key, archive_receipt)
                    .map_err(operation)?
                {
                    TransitionOutcome::Durable(status) | TransitionOutcome::Idempotent(status) => {
                        Ok(status)
                    }
                }
            })?;
            if capture.branch_label.is_some() {
                self.select_automatic_branch_for_submitter(owner_id, &key, &submission, &capture)?;
            }
        }
        if status.state == SubmissionState::Committed && status.post_commit_receipt.is_none() {
            let receipt = status
                .archive_receipt
                .as_ref()
                .ok_or_else(|| operation("committed entry lacks Archive receipt"))?;
            let post = with_operation(self, &operation_authority, |runtime| {
                ensure_session_at(
                    runtime,
                    key.conversation_id.as_str(),
                    Some(&receipt.node_id),
                )?;
                let result = runtime
                    .schedule_session(key.conversation_id.as_str(), self.episode_policy, now_ns())
                    .map_err(operation)?;
                Ok(PostCommitReceipt {
                    operation_id: format!(
                        "{owner_id}/{}/{}",
                        key.conversation_id.as_str(),
                        key.submission_id.as_str()
                    ),
                    scheduler_receipt: format!("{result:?}"),
                })
            })?;
            status = with_accepted_work(self, &operation_authority, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                compact_if_needed(&mut journal)?;
                match journal
                    .record_postcommit_receipt(&key, post)
                    .map_err(operation)?
                {
                    TransitionOutcome::Durable(status) | TransitionOutcome::Idempotent(status) => {
                        Ok(status)
                    }
                }
            })?;
        }
        Ok(GatewaySubmissionReceipt { id: key, status })
    }

    fn select_automatic_branch_for_submitter(
        &self,
        owner_id: &str,
        key: &ConversationSubmissionId,
        submission: &Submission,
        capture: &FinalizeCapture,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        if capture.branch_label.is_none() {
            return Ok(());
        }
        let generation_id = conversation_host_operation_id(owner_id, key);
        let instance_id = {
            let operations = self.conversation.owner_journal(owner_id)?;
            let operations = operations
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let Some(attribution) = operations.attribution(&generation_id) else {
                return Ok(());
            };
            if attribution.conversation_id != key.conversation_id.as_str()
                || attribution.submission_id != key.submission_id.as_str()
                || attribution.actor_principal.as_deref() != Some(submission.principal_id.as_str())
            {
                return Err(operation(
                    "automatic branch submitter attribution conflicts with durable admission",
                ));
            }
            match Uuid::parse_str(&attribution.admission.instance_id) {
                Ok(id) => id,
                Err(_) => return Ok(()),
            }
        };
        let Ok(attachment) = self.attachment_by_id(instance_id) else {
            return Ok(());
        };
        if attachment.instance.principal_id() != submission.principal_id {
            return Err(operation(
                "automatic branch attachment principal differs from durable admission",
            ));
        }
        // Selection is advisory instance-local state. A detached or revoked
        // submitter must not block already-accepted durable finalization.
        let _ = self.select_managed_conversation(
            &attachment.instance,
            owner_id,
            key.conversation_id.as_str(),
            &capture.branch_id,
        );
        Ok(())
    }
}

fn compact_if_needed(journal: &mut RelJournal) -> Result<(), ReliquaryRuntimeHostError> {
    if journal.needs_compaction().map_err(operation)? {
        journal.compact().map_err(operation)?;
    }
    Ok(())
}

fn conversation_host_operation_id(owner_id: &str, key: &ConversationSubmissionId) -> String {
    let mut digest = Sha256::new();
    for component in [
        owner_id,
        key.conversation_id.as_str(),
        key.submission_id.as_str(),
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    let suffix = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("reliquary/conversation-execution/{suffix}")
}

fn conversation_host_operation(
    owner_id: &str,
    key: &ConversationSubmissionId,
    _submission: &Submission,
    journal: &RelJournal,
) -> Result<HostOperation, ReliquaryRuntimeHostError> {
    let (payload, payload_digest) = journal.host_payload(owner_id, key).map_err(operation)?;
    Ok(HostOperation {
        stable_id: conversation_host_operation_id(owner_id, key),
        payload_digest,
        payload,
    })
}

fn drive_host_conversation_stage(
    gateway: &GatewayRuntime,
    authority: &OperationAuthority,
    owner_id: &str,
    journal: &Arc<Mutex<RelJournal>>,
    key: &ConversationSubmissionId,
    submission: &Submission,
) -> Result<Option<SubmissionStatus>, ReliquaryRuntimeHostError> {
    let capability = RuntimeCapability::ConversationExecution;
    let (current_operation, current_status) = {
        let journal = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        (
            journal.host_operation(key, capability),
            journal
                .status(key)
                .ok_or_else(|| operation("submission not found"))?,
        )
    };
    // Durable terminal proof is authoritative even after the adapter disappears
    // or a sticky cancellation request was recorded before completion won.
    if let Some(record) = current_operation.as_ref() {
        if matches!(
            record.phase,
            DurableHostOperationPhase::Completed | DurableHostOperationPhase::Cancelled
        ) {
            with_accepted_work(gateway, authority, || {
                if record.phase == DurableHostOperationPhase::Cancelled {
                    let mut journal = journal
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    if journal
                        .status(key)
                        .is_some_and(|s| s.state == SubmissionState::Pending)
                    {
                        let revision = journal.conversation_revision(&key.conversation_id);
                        journal.cancel(key, revision).map_err(operation)?;
                    }
                }
                Ok(())
            })?;
            if let Some(router) = gateway.host_router.as_ref() {
                router
                    .release_operation(capability, &record.stable_id)
                    .map_err(operation)?;
            }
            return if record.phase == DurableHostOperationPhase::Completed {
                Ok(None)
            } else {
                Ok(Some(
                    journal
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                        .status(key)
                        .ok_or_else(|| operation("submission not found"))?,
                ))
            };
        }
    }
    let Some(router) = gateway.host_router.as_ref() else {
        if current_operation.is_some() {
            with_accepted_work(gateway, authority, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                journal
                    .record_host_status(key, capability, HostOperationStatus::Unknown)
                    .map_err(operation)?;
                Ok(())
            })?;
            return Ok(Some(
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .status(key)
                    .ok_or_else(|| operation("submission not found"))?,
            ));
        }
        return Ok(None);
    };

    if let Some(operation_record) = current_operation {
        router
            .remember_operation(capability, operation_record.stable_id.clone())
            .map_err(operation)?;
        // Sticky cancellation survives disconnects and Unknown status. Reconcile it
        // before any handoff retry or native finalization can occur.
        if operation_record.cancel_requested {
            let status_result = {
                with_accepted_work(gateway, authority, || Ok(()))?;
                router.status(capability, &operation_record.stable_id)
            };
            let status = match status_result {
                Ok(status) => status,
                Err(_) => {
                    with_accepted_work(gateway, authority, || {
                        let mut journal = journal
                            .lock()
                            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                        journal
                            .record_host_status(key, capability, HostOperationStatus::Unknown)
                            .map_err(operation)?;
                        Ok(())
                    })?;
                    return Ok(Some(
                        journal
                            .lock()
                            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                            .status(key)
                            .ok_or_else(|| operation("submission not found"))?,
                    ));
                }
            };
            if matches!(
                status,
                HostOperationStatus::Pending
                    | HostOperationStatus::Running
                    | HostOperationStatus::Unknown
            ) {
                let cancel_outcome = if status == HostOperationStatus::Unknown {
                    Err(super::HostContractError::HostCancelUnknown)
                } else {
                    with_accepted_work(gateway, authority, || Ok(()))?;
                    router.cancel(capability, &operation_record.stable_id)
                };
                let terminal_cancel = matches!(
                    cancel_outcome.as_ref(),
                    Ok(HostCancelOutcome::CancelledBeforeStart)
                        | Ok(HostCancelOutcome::AlreadyTerminal(
                            HostOperationStatus::Cancelled
                        ))
                );
                let terminal_complete = matches!(
                    cancel_outcome.as_ref(),
                    Ok(HostCancelOutcome::AlreadyTerminal(
                        HostOperationStatus::Completed
                    ))
                );
                with_accepted_work(gateway, authority, || {
                    let mut journal = journal
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    match cancel_outcome {
                        Ok(outcome) => {
                            journal
                                .record_host_cancel(key, capability, outcome)
                                .map_err(operation)?;
                            if terminal_cancel
                                && journal
                                    .status(key)
                                    .is_some_and(|s| s.state == SubmissionState::Pending)
                            {
                                let revision = journal.conversation_revision(&key.conversation_id);
                                journal.cancel(key, revision).map_err(operation)?;
                            }
                        }
                        Err(_) => {
                            journal
                                .record_host_status(key, capability, HostOperationStatus::Unknown)
                                .map_err(operation)?;
                        }
                    }
                    Ok(())
                })?;
                if terminal_cancel || terminal_complete {
                    router
                        .release_operation(capability, &operation_record.stable_id)
                        .map_err(operation)?;
                }
                if terminal_complete {
                    return Ok(None);
                }
                return Ok(Some(
                    journal
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                        .status(key)
                        .ok_or_else(|| operation("submission not found"))?,
                ));
            }
            with_accepted_work(gateway, authority, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                journal
                    .record_host_status(key, capability, status)
                    .map_err(operation)?;
                if status == HostOperationStatus::Cancelled
                    && journal
                        .status(key)
                        .is_some_and(|s| s.state == SubmissionState::Pending)
                {
                    let revision = journal.conversation_revision(&key.conversation_id);
                    journal.cancel(key, revision).map_err(operation)?;
                }
                Ok(())
            })?;
            let updated = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .status(key)
                .ok_or_else(|| operation("submission not found"))?;
            if matches!(
                status,
                HostOperationStatus::Cancelled | HostOperationStatus::Completed
            ) {
                router
                    .release_operation(capability, &operation_record.stable_id)
                    .map_err(operation)?;
            }
            return if status == HostOperationStatus::Completed {
                Ok(None)
            } else {
                Ok(Some(updated))
            };
        }

        match operation_record.phase {
            DurableHostOperationPhase::HandoffIntent => {
                let host_operation = {
                    let journal = journal
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    conversation_host_operation(owner_id, key, submission, &journal)?
                };
                if host_operation.stable_id != operation_record.stable_id
                    || host_operation.payload_digest != operation_record.payload_digest
                {
                    return Err(operation("durable host handoff payload changed"));
                }
                with_accepted_work(gateway, authority, || Ok(()))?;
                let receipt =
                    if router.authority(capability).map_err(operation)? == StageAuthority::Held {
                        router.retry_handoff(capability, &host_operation)
                    } else {
                        router.handoff(capability, &host_operation)
                    };
                with_accepted_work(gateway, authority, || {
                    let mut journal = journal
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    match receipt {
                        Ok(ref receipt) => journal
                            .record_host_handoff(key, capability, receipt)
                            .map_err(operation)?,
                        Err(_) => journal
                            .record_host_status(key, capability, HostOperationStatus::Unknown)
                            .map_err(operation)?,
                    };
                    Ok(())
                })?;
                let updated = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .status(key)
                    .ok_or_else(|| operation("submission not found"))?;
                return Ok(Some(updated));
            }
            _ => {}
        }

        let status = {
            with_accepted_work(gateway, authority, || Ok(()))?;
            router.status(capability, &operation_record.stable_id)
        };
        let status = match status {
            Ok(status) => status,
            Err(_) => HostOperationStatus::Unknown,
        };
        with_accepted_work(gateway, authority, || {
            let mut journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            journal
                .record_host_status(key, capability, status)
                .map_err(operation)?;
            if status == HostOperationStatus::Cancelled
                && journal
                    .status(key)
                    .is_some_and(|s| s.state == SubmissionState::Pending)
            {
                let revision = journal.conversation_revision(&key.conversation_id);
                journal.cancel(key, revision).map_err(operation)?;
            }
            Ok(())
        })?;
        let updated = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(key)
            .ok_or_else(|| operation("submission not found"))?;
        if matches!(
            status,
            HostOperationStatus::Completed | HostOperationStatus::Cancelled
        ) {
            router
                .release_operation(capability, &operation_record.stable_id)
                .map_err(operation)?;
        }
        return if status == HostOperationStatus::Completed {
            Ok(None)
        } else {
            Ok(Some(updated))
        };
    }

    match router.authority(capability).map_err(operation)? {
        StageAuthority::Native => Ok(None),
        StageAuthority::Held => Ok(Some(current_status)),
        StageAuthority::Host => {
            let host_operation = {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                conversation_host_operation(owner_id, key, submission, &journal)?
            };
            with_accepted_work(gateway, authority, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                journal
                    .begin_host_handoff(
                        key,
                        capability,
                        host_operation.stable_id.clone(),
                        host_operation.payload_digest,
                    )
                    .map_err(operation)?;
                router
                    .remember_operation(capability, host_operation.stable_id.clone())
                    .map_err(operation)?;
                Ok(())
            })?;
            with_accepted_work(gateway, authority, || Ok(()))?;
            let receipt = router.handoff(capability, &host_operation);
            with_accepted_work(gateway, authority, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                match receipt {
                    Ok(ref receipt) => journal
                        .record_host_handoff(key, capability, receipt)
                        .map_err(operation)?,
                    Err(_) => journal
                        .record_host_status(key, capability, HostOperationStatus::Unknown)
                        .map_err(operation)?,
                };
                Ok(())
            })?;
            let updated = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .status(key)
                .ok_or_else(|| operation("submission not found"))?;
            Ok(Some(updated))
        }
    }
}

fn with_admission<T>(
    gateway: &GatewayRuntime,
    token: &OwnerAccessToken,
    f: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
) -> Result<T, ReliquaryRuntimeHostError> {
    let attachment = gateway.attachment_by_id(token.instance_id)?;
    let attachment_lease = attachment.leases.acquire_current()?;
    attachment
        .instance
        .authorize_owner_generation(
            &token.owner_id,
            token.grant,
            token.authorization,
            token.owner_grant_generation,
        )
        .map_err(operation)?;
    let (mount_epoch, fence) = match token.kind {
        super::OwnerKind::Rel => {
            let owners = gateway
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let owner = owners
                .get(&token.owner_id)
                .ok_or_else(|| operation("conversation REL is not mounted"))?;
            (
                owner.mount_epoch.load(std::sync::atomic::Ordering::SeqCst),
                Arc::clone(&owner.fence),
            )
        }
        super::OwnerKind::Phy => {
            if token.grant != OwnerGrant::UseOwnPhy {
                return Err(operation("personal admission lacks UseOwnPhy"));
            }
            let owners = gateway
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let owner = owners
                .get(&token.owner_id)
                .ok_or_else(|| operation("admission PHY is not mounted"))?;
            (
                owner.mount_epoch.load(std::sync::atomic::Ordering::SeqCst),
                Arc::clone(&owner.fence),
            )
        }
    };
    if mount_epoch != token.mount_epoch {
        return Err(operation(
            "admission token belongs to a previous owner mount",
        ));
    }
    let owner_lease = fence.acquire(token.fence_generation)?;
    attachment
        .instance
        .authorize_owner_generation(
            &token.owner_id,
            token.grant,
            token.authorization,
            token.owner_grant_generation,
        )
        .map_err(operation)?;
    // No InteractionRuntime/store lock is held while the REL journal syncs.
    let result = f();
    drop(owner_lease);
    drop(attachment_lease);
    result
}

fn with_accepted_work<T>(
    gateway: &GatewayRuntime,
    authority: &OperationAuthority,
    f: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
) -> Result<T, ReliquaryRuntimeHostError> {
    if gateway
        .shutting_down
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err(operation("gateway is shutting down"));
    }
    let rels = gateway
        .rel_executions
        .read()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    let rel = rels
        .get(&authority.owner_id)
        .ok_or_else(|| operation("conversation REL is not mounted"))?;
    if rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst) != authority.mount_epoch {
        return Err(operation("operation belongs to a previous REL mount"));
    }
    if rel.fence.snapshot()?.0 != OwnerAdmission::Open {
        return Err(operation("conversation REL is quiesced"));
    }
    let lease = rel.fence.acquire(authority.fence_generation)?;
    drop(rels);
    let result =
        gateway.with_conversation_distribution(&authority.owner_id, authority.mount_epoch, f);
    drop(lease);
    result
}

fn with_conversation_admission<T>(
    gateway: &GatewayRuntime,
    token: &OwnerAccessToken,
    f: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
) -> Result<T, ReliquaryRuntimeHostError> {
    with_admission(gateway, token, || match token.kind {
        super::OwnerKind::Rel => {
            gateway.with_conversation_distribution(&token.owner_id, token.mount_epoch, f)
        }
        super::OwnerKind::Phy => f(),
    })
}

fn with_operation<T>(
    gateway: &GatewayRuntime,
    authority: &OperationAuthority,
    f: impl FnOnce(&mut crate::InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
) -> Result<T, ReliquaryRuntimeHostError> {
    with_accepted_work(gateway, authority, || {
        let owner = {
            let rels = gateway
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Arc::clone(
                &rels
                    .get(&authority.owner_id)
                    .ok_or_else(|| operation("conversation REL is not mounted"))?
                    .owner,
            )
        };
        let runtime = owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .runtime
            .as_ref()
            .cloned()
            .ok_or_else(|| operation("conversation REL runtime is unavailable"))?;
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        f(&mut runtime)
    })
}

fn validate_accepted_submission(
    owner_id: &str,
    key: &ConversationSubmissionId,
    submission: &Submission,
) -> Result<(), ReliquaryRuntimeHostError> {
    let scope = fingerprint(&json!({
        "rel_id": owner_id, "conversation_id": key.conversation_id.as_str(),
        "branch_id": metadata_branch(submission),
    }))?;
    if submission.id != *key
        || submission.principal_id.is_empty()
        || submission.scope_fingerprint != scope
    {
        return Err(operation("durable admission identity or scope changed"));
    }
    // Earlier admissions already retained verified principal and owner scope.
    if let Some(admission) = submission.request_metadata.get("admission") {
        if admission.get("owner_id").and_then(|v| v.as_str()) != Some(owner_id)
            || admission.get("principal_id").and_then(|v| v.as_str())
                != Some(submission.principal_id.as_str())
            || admission.get("grant").and_then(|v| v.as_str()) != Some("SubmitConversation")
            || admission.get("policy_revision").and_then(|v| v.as_u64()) != Some(1)
        {
            return Err(operation("durable admission authorization changed"));
        }
    }
    Ok(())
}

fn validate_finalize_identity(
    submission: &Submission,
    capture: &FinalizeCapture,
) -> Result<(), ReliquaryRuntimeHostError> {
    if capture.verified_principal_id != submission.principal_id || capture.policy_revision == 0 {
        return Err(operation(
            "finalization differs from durable admitted identity",
        ));
    }

    let admitted_branch = metadata_branch(submission);
    if capture.branch_id == admitted_branch {
        if capture.branch_label.is_some() {
            return Err(operation(
                "caller-selected branch cannot carry an automatic branch label",
            ));
        }
        return Ok(());
    }

    let (expected_branch, expected_label) = finalization::automatic_branch_identity(submission)?;
    if capture.branch_id != expected_branch
        || capture.branch_label.as_deref() != Some(expected_label.as_str())
        || capture.parent_node_id != finalization::expected_parent(submission)?
    {
        return Err(operation(
            "automatic branch finalization differs from durable conflict identity",
        ));
    }
    Ok(())
}

fn archive_receipt_or_commit(
    runtime: &mut crate::InteractionRuntime,
    key: &ConversationSubmissionId,
    submission: &Submission,
    capture: &FinalizeCapture,
    attachment_bytes: &[(AttachmentIdentity, Vec<u8>)],
) -> Result<ArchiveReceipt, ReliquaryRuntimeHostError> {
    if submission.id != *key {
        return Err(operation(
            "Archive operation differs from durable submission identity",
        ));
    }
    validate_finalize_identity(submission, capture)?;
    finalization::validate_publication_head(runtime, key, capture)?;
    let stable_id = key.submission_id.as_str();
    if runtime
        .cva()
        .archive()
        .has_node(key.conversation_id.as_str(), stable_id)
    {
        let turn = runtime
            .conversation_turns(key.conversation_id.as_str(), stable_id)
            .map_err(operation)?
            .into_iter()
            .find(|t| t.node_id == stable_id)
            .ok_or_else(|| operation("stable Archive node is not in its own ancestry"))?;
        validate_durable_turn(runtime, &turn, submission, capture)?;
        restore_session_after_commit(
            runtime,
            key.conversation_id.as_str(),
            stable_id,
            capture.parent_node_id.as_deref(),
        )?;
        publish_branch(
            runtime,
            key.conversation_id.as_str(),
            &capture.branch_id,
            stable_id,
            capture.branch_label.is_none(),
        )?;
        validate_archive_branch(
            runtime,
            key.conversation_id.as_str(),
            &capture.branch_id,
            stable_id,
        )?;
    } else {
        ensure_session_at(
            runtime,
            key.conversation_id.as_str(),
            capture.parent_node_id.as_deref(),
        )?;
        if runtime
            .session(key.conversation_id.as_str())
            .is_some_and(|session| session.message_in_progress.is_some())
        {
            return Err(operation(
                "cannot commit a conversation turn while another message is in flight",
            ));
        }
        let attachments = attachment_bytes
            .iter()
            .map(|(id, bytes)| InteractionAttachment {
                filename: id.filename.clone(),
                mime_type: id.mime_type.clone(),
                bytes: bytes.clone(),
            })
            .collect();
        let turn = InteractionTurn {
            message_id: stable_id.to_owned(),
            session_id: key.conversation_id.as_str().to_owned(),
            parent_message_id: capture.parent_node_id.clone(),
            role: InteractionRole::User,
            principal_id: Some(capture.verified_principal_id.clone()),
            timestamp_ns: capture.committed_timestamp_ns,
            content: submission.content.clone(),
            attachments,
            project_attachments: Vec::<StoredFile>::new(),
        };
        let accepted = runtime.accept_turn(turn).map_err(operation)?;
        if accepted.turn.node.id != stable_id {
            return Err(operation("Archive accepted a different Node ID"));
        }
        let state = runtime
            .sessions
            .get_mut(key.conversation_id.as_str())
            .ok_or_else(|| operation("shared continuation session disappeared during commit"))?;
        if state.in_flight.is_some() {
            return Err(operation("cannot commit while a message is in flight"));
        }
        if state.leaf_message_id != capture.parent_node_id {
            return Err(operation(
                "captured Archive parent changed before durable acceptance",
            ));
        }
        state.leaf_message_id = Some(stable_id.to_owned());
        let durable = runtime
            .conversation_turns(key.conversation_id.as_str(), stable_id)
            .map_err(operation)?
            .into_iter()
            .find(|t| t.node_id == stable_id)
            .ok_or_else(|| operation("accepted Node is absent from Archive ancestry"))?;
        validate_durable_turn(runtime, &durable, submission, capture)?;
        publish_branch(
            runtime,
            key.conversation_id.as_str(),
            &capture.branch_id,
            stable_id,
            capture.branch_label.is_none(),
        )?;
        validate_archive_branch(
            runtime,
            key.conversation_id.as_str(),
            &capture.branch_id,
            stable_id,
        )?;
    }
    Ok(ArchiveReceipt {
        node_id: stable_id.to_owned(),
        branch_id: capture.branch_id.clone(),
        parent_node_id: capture.parent_node_id.clone(),
        principal_id: capture.verified_principal_id.clone(),
        committed_timestamp_ns: capture.committed_timestamp_ns,
        payload_hash: submission.payload_hash.clone(),
        attachment_identities: submission.attachments.clone(),
    })
}
fn validate_durable_turn(
    runtime: &crate::InteractionRuntime,
    turn: &ResolvedTurn,
    submission: &Submission,
    capture: &FinalizeCapture,
) -> Result<(), ReliquaryRuntimeHostError> {
    validate_finalize_identity(submission, capture)?;
    let conversation_id = submission.id.conversation_id.as_str();
    let node = runtime
        .cva()
        .archive()
        .nodes
        .get(conversation_id, &turn.node_id)
        .ok_or_else(|| operation("stable Archive Node is absent from the node index"))?;
    let expected_content_hash = hex(&Sha256::digest(submission.content.as_bytes()));
    if node.id != submission.id.submission_id.as_str()
        || node.conversation_id != conversation_id
        || node.parent_id != capture.parent_node_id
        || node.role != "user"
        || node.principal_id.as_deref() != Some(capture.verified_principal_id.as_str())
        || node.timestamp_ns != capture.committed_timestamp_ns
        || hex(&node.content_id.0) != expected_content_hash
        || turn.node_id != node.id
        || turn.content != submission.content
    {
        return Err(operation(
            "stable Archive Node differs from captured conversation, parent, principal, time, or content",
        ));
    }
    let actual = runtime
        .cva()
        .files_for_source(conversation_id, &turn.node_id);
    if actual.len() != submission.attachments.len()
        || actual
            .iter()
            .zip(&submission.attachments)
            .any(|(file, id)| {
                file.filename != id.filename
                    || file.mime_type != id.mime_type
                    || hex(&file.content_id.0) != id.content_hash
            })
    {
        return Err(operation(
            "stable Archive attachment set differs from captured submission",
        ));
    }
    Ok(())
}
fn validate_archive_branch(
    runtime: &crate::InteractionRuntime,
    conversation_id: &str,
    branch_id: &str,
    leaf_node_id: &str,
) -> Result<(), ReliquaryRuntimeHostError> {
    if runtime.cva().archive().branches().iter().any(|branch| {
        branch.conversation_id == conversation_id
            && branch.id == branch_id
            && branch.leaf_node_id == leaf_node_id
    }) {
        Ok(())
    } else {
        Err(operation(
            "stable Archive branch does not point at the committed node",
        ))
    }
}
fn publish_branch(
    runtime: &mut crate::InteractionRuntime,
    conversation_id: &str,
    branch_id: &str,
    leaf_node_id: &str,
    canonical_if_new: bool,
) -> Result<(), ReliquaryRuntimeHostError> {
    let desired = Branch {
        id: branch_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        leaf_node_id: leaf_node_id.to_owned(),
        canonical: runtime
            .cva()
            .archive()
            .branches()
            .iter()
            .find(|branch| branch.conversation_id == conversation_id && branch.id == branch_id)
            .map(|branch| branch.canonical)
            .unwrap_or(canonical_if_new),
    };
    if runtime
        .cva()
        .archive()
        .branches()
        .iter()
        .any(|branch| branch == &desired)
    {
        // A previous append may have reached the in-memory index before sync failed.
        // A receipt requires durability even when the desired branch already exists.
        return runtime.cva().sync().map_err(operation);
    }
    runtime.cva.append_branch(desired).map_err(operation)?;
    runtime.cva().sync().map_err(operation)
}

fn with_internal_runtime<T>(
    gateway: &GatewayRuntime,
    owner_id: &str,
    f: impl FnOnce(&mut crate::InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
) -> Result<T, ReliquaryRuntimeHostError> {
    gateway.with_quiesced_rel_during_transition(owner_id, f)
}

fn ensure_session_at(
    runtime: &mut crate::InteractionRuntime,
    conversation_id: &str,
    parent: Option<&str>,
) -> Result<(), ReliquaryRuntimeHostError> {
    if let Some(session) = runtime.session(conversation_id) {
        if session.message_in_progress.is_some() {
            return Err(operation("cannot rebind an in-flight continuation"));
        }
        if let Some(parent) = parent {
            if !runtime.cva().archive().has_node(conversation_id, parent) {
                return Err(operation("continuation parent is absent from Archive"));
            }
        }
        // This is a temporary cursor under the one owner runtime lock. Branch
        // heads remain Archive-owned and the coordinator owns branch leases.
        runtime
            .sessions
            .get_mut(conversation_id)
            .ok_or_else(|| operation("shared continuation session disappeared"))?
            .leaf_message_id = parent.map(str::to_owned);
        return Ok(());
    }
    runtime
        .open_session(conversation_id.to_owned(), parent.map(str::to_owned))
        .map_err(operation)?;
    Ok(())
}
fn restore_session_after_commit(
    runtime: &mut crate::InteractionRuntime,
    conversation_id: &str,
    stable_id: &str,
    _captured_parent: Option<&str>,
) -> Result<(), ReliquaryRuntimeHostError> {
    ensure_session_at(runtime, conversation_id, Some(stable_id))
}
fn metadata_branch(submission: &Submission) -> String {
    submission
        .request_metadata
        .get("branch_id")
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .to_owned()
}
fn fingerprint(value: &serde_json::Value) -> Result<String, ReliquaryRuntimeHostError> {
    let bytes = serde_json::to_vec(value).map_err(operation)?;
    Ok(hex(&Sha256::digest(bytes)))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(i64::MAX as u128) as i64
}

#[path = "runtime_host_gateway_conversation_finalization.rs"]
mod finalization;

#[cfg(test)]
#[path = "runtime_host_gateway_conversation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_host_gateway_host_operation_tests.rs"]
mod host_operation_tests;

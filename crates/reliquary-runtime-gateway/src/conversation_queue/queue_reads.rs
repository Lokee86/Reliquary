use super::*;

impl ConversationQueue {
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn sequence(&self) -> u64 {
        self.projection.sequence
    }
    pub fn conversation_revision(&self, conversation_id: &str) -> u64 {
        self.projection.revision(conversation_id)
    }
    pub(crate) fn status(&self, id: &ConversationSubmissionId) -> Option<SubmissionStatus> {
        self.projection.entries.get(id).map(Entry::status)
    }
    pub(crate) fn host_operation(
        &self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Option<DurableHostOperation> {
        self.projection
            .entries
            .get(id)?
            .host_operations
            .get(&capability)
            .cloned()
    }
    pub(crate) fn generation_checkpoint(
        &self,
        id: &ConversationSubmissionId,
    ) -> Option<DurableGenerationCheckpoint> {
        self.projection
            .entries
            .get(id)?
            .generation
            .as_ref()?
            .checkpoint
            .clone()
    }
    pub(crate) fn replay_generation_checkpoints(
        &self,
        id: &ConversationSubmissionId,
        after_cursor: u64,
        max_items: usize,
    ) -> Result<GenerationReplay, QueueError> {
        if max_items == 0 || max_items > MAX_REPLAY_CHECKPOINTS {
            return Err(QueueError::InvalidReplayBound);
        }
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        let latest_cursor = entry
            .generation
            .as_ref()
            .and_then(|generation| generation.checkpoint.as_ref())
            .map_or(0, |checkpoint| checkpoint.delivery_cursor);
        if after_cursor > latest_cursor {
            return Err(QueueError::InvalidReplayCursor);
        }
        let Some(first) = entry.checkpoint_history.front() else {
            return Ok(GenerationReplay::Checkpoints {
                latest_cursor,
                items: Vec::new(),
            });
        };
        if after_cursor.saturating_add(1) < first.delivery_cursor {
            return Ok(GenerationReplay::ResyncRequired { latest_cursor });
        }
        let items: Vec<_> = entry
            .checkpoint_history
            .iter()
            .filter(|checkpoint| checkpoint.delivery_cursor > after_cursor)
            .cloned()
            .collect();
        let bytes: usize = items.iter().map(|item| item.content_prefix.len()).sum();
        if items.len() > max_items || bytes > MAX_REPLAY_BYTES {
            return Ok(GenerationReplay::ResyncRequired { latest_cursor });
        }
        Ok(GenerationReplay::Checkpoints {
            latest_cursor,
            items,
        })
    }
    pub(crate) fn replay_generation_events(
        &self,
        id: &ConversationSubmissionId,
        after_cursor: u64,
        max_items: usize,
    ) -> Result<GenerationEventReplay, QueueError> {
        if max_items == 0 || max_items > MAX_REPLAY_CHECKPOINTS {
            return Err(QueueError::InvalidReplayBound);
        }
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        let latest_cursor = entry
            .generation
            .as_ref()
            .map_or(0, |generation| generation.stream_cursor);
        if after_cursor > latest_cursor {
            return Err(QueueError::InvalidReplayCursor);
        }
        let Some(first) = entry.generation_event_history.front() else {
            return if after_cursor == latest_cursor {
                Ok(GenerationEventReplay::Events {
                    latest_cursor,
                    items: Vec::new(),
                })
            } else {
                Ok(GenerationEventReplay::ResyncRequired { latest_cursor })
            };
        };
        if after_cursor.saturating_add(1) < first.cursor {
            return Ok(GenerationEventReplay::ResyncRequired { latest_cursor });
        }
        let items: Vec<_> = entry
            .generation_event_history
            .iter()
            .filter(|event| event.cursor > after_cursor)
            .cloned()
            .collect();
        let bytes: usize = items
            .iter()
            .filter_map(|event| event.checkpoint.as_ref())
            .map(|checkpoint| checkpoint.content_prefix.len())
            .sum();
        if items.len() > max_items || bytes > MAX_REPLAY_BYTES {
            return Ok(GenerationEventReplay::ResyncRequired { latest_cursor });
        }
        Ok(GenerationEventReplay::Events {
            latest_cursor,
            items,
        })
    }
    #[allow(dead_code)] // retained for the future authorized conversation reader
    pub(crate) fn submission(&self, id: &ConversationSubmissionId) -> Option<&QueuedSubmission> {
        self.projection
            .entries
            .get(id)
            .filter(|e| e.state == QueueState::Pending || e.state == QueueState::Finalizing)
            .map(|e| &e.submission)
    }
    pub(crate) fn pending_in_order(
        &self,
        conversation_id: &str,
    ) -> Vec<(ConversationSubmissionId, SubmissionStatus)> {
        let mut pending: Vec<_> = self
            .projection
            .entries
            .iter()
            .filter(|(id, entry)| {
                id.conversation_id == conversation_id
                    && entry.state == QueueState::Pending
                    && entry.host_operations.is_empty()
                    && entry.generation.is_none()
            })
            .map(|(id, entry)| (id.clone(), entry.status()))
            .collect();
        pending.sort_by_key(|(_, status)| status.admission_sequence);
        pending
    }
    pub(crate) fn submitter_principal(&self, id: &ConversationSubmissionId) -> Option<&str> {
        self.projection
            .entries
            .get(id)
            .map(|entry| entry.submission.principal_id.as_str())
    }
}

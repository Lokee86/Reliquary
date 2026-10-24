use crate::{InteractionError, InteractionSession, RelExecution};

pub(crate) struct InFlightMessage {
    pub(crate) message_id: String,
    pub(crate) parent_message_id: Option<String>,
    pub(crate) role: crate::InteractionRole,
    pub(crate) principal_id: Option<String>,
    pub(crate) timestamp_ns: i64,
    pub(crate) content: String,
    pub(crate) attachments: Vec<crate::InteractionAttachment>,
    pub(crate) project_attachments: Vec<crate::StoredFile>,
}

pub(crate) struct SessionState {
    pub(crate) leaf_message_id: Option<String>,
    pub(crate) in_flight: Option<InFlightMessage>,
}

impl RelExecution {
    pub fn open_session(
        &mut self,
        session_id: String,
        resume_from: Option<String>,
    ) -> Result<InteractionSession, InteractionError> {
        validate_id(&session_id, "session id")?;
        if self.sessions.contains_key(&session_id) {
            return Err(InteractionError::SessionAlreadyOpen);
        }
        match resume_from.as_deref() {
            Some(message_id) => {
                validate_id(message_id, "resume message id")?;
                if !self.cva.owner.archive.has_node(&session_id, message_id) {
                    return Err(InteractionError::MissingResumeMessage);
                }
            }
            None if self.cva.owner.archive.has_conversation(&session_id) => {
                return Err(InteractionError::ResumeRequired);
            }
            None => {}
        }
        self.sessions.insert(
            session_id.clone(),
            SessionState {
                leaf_message_id: resume_from,
                in_flight: None,
            },
        );
        self.session(&session_id)
            .ok_or(InteractionError::UnknownSession)
    }

    pub fn session(&self, session_id: &str) -> Option<InteractionSession> {
        self.sessions
            .get(session_id)
            .map(|state| InteractionSession {
                session_id: session_id.to_owned(),
                leaf_message_id: state.leaf_message_id.clone(),
                message_in_progress: state
                    .in_flight
                    .as_ref()
                    .map(|message| message.message_id.clone()),
            })
    }

    pub fn close_session(
        &mut self,
        session_id: &str,
    ) -> Result<InteractionSession, InteractionError> {
        let state = self
            .sessions
            .get(session_id)
            .ok_or(InteractionError::UnknownSession)?;
        if state.in_flight.is_some() {
            return Err(InteractionError::MessageInProgress);
        }
        let state = self
            .sessions
            .remove(session_id)
            .ok_or(InteractionError::UnknownSession)?;
        Ok(InteractionSession {
            session_id: session_id.to_owned(),
            leaf_message_id: state.leaf_message_id,
            message_in_progress: None,
        })
    }
}

pub(crate) fn validate_id(value: &str, field: &'static str) -> Result<(), InteractionError> {
    if value.is_empty() {
        Err(InteractionError::InvalidField(field))
    } else {
        Ok(())
    }
}

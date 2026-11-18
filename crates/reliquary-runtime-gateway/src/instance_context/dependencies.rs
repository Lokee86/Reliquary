use super::*;
use crate::instance_owner::RelMount;
use reliquary_application::{
    ContextMessage, MAX_CONTEXT_SOURCE_BYTES, MAX_CONTEXT_SOURCE_TURNS,
    rel_synthesis_context_message,
};

#[derive(Eq, PartialEq)]
pub(super) struct ContextRelDependencyBinding {
    owner: OwnerId,
    mount: RelMount,
    generation: u64,
    memory_version: Option<u64>,
    synthesis_revision: Option<u64>,
}

impl Instance {
    pub(super) fn capture_rel_dependency_context(
        &self,
        owner: OwnerId,
        mut turns: usize,
        mut bytes: usize,
    ) -> Result<(Vec<ContextRelDependencyBinding>, Vec<ContextMessage>), GatewayError> {
        let mut messages = Vec::new();
        let bindings = self.read_rel_dependency_context(owner, |dependency, revision, text| {
            let message = rel_synthesis_context_message(dependency.0, revision, text);
            turns = turns.saturating_add(1);
            bytes = bytes.saturating_add(message.content.len());
            if turns > MAX_CONTEXT_SOURCE_TURNS {
                return Err(GatewayError::ContextDependencyBounds(
                    "context source turn limit exceeded".into(),
                ));
            }
            if bytes > MAX_CONTEXT_SOURCE_BYTES {
                return Err(GatewayError::ContextDependencyBounds(
                    "context source byte limit exceeded".into(),
                ));
            }
            messages.push(message);
            Ok(())
        })?;
        Ok((bindings, messages))
    }

    pub(super) fn recheck_rel_dependency_context(
        &self,
        owner: OwnerId,
        expected: &[ContextRelDependencyBinding],
    ) -> Result<(), GatewayError> {
        let current = self
            .read_rel_dependency_context(owner, |_, _, _| Ok(()))
            .map_err(|error| match error {
                GatewayError::RelDependencyUnresolved | GatewayError::RelDependencyCycle => {
                    GatewayError::StaleRelContext
                }
                error => error,
            })?;
        if current != expected {
            return Err(GatewayError::StaleRelContext);
        }
        Ok(())
    }

    fn read_rel_dependency_context(
        &self,
        primary: OwnerId,
        mut read_synthesis: impl FnMut(OwnerId, u64, &str) -> Result<(), GatewayError>,
    ) -> Result<Vec<ContextRelDependencyBinding>, GatewayError> {
        self.with_rel_dependency_order(primary, |order| {
            let mut bindings = Vec::with_capacity(order.len());
            for owner in order {
                let mount = RelMount::capture(self, owner)?;
                let binding = self.with_reliquary(owner, |session| {
                    let mut memory_version = None;
                    let mut synthesis_revision = None;
                    if owner != primary {
                        memory_version = Some(session.memory_version());
                        if let Some(synthesis) = session.current_ego_synthesis() {
                            read_synthesis(owner, synthesis.revision, &synthesis.text)?;
                            synthesis_revision = Some(synthesis.revision);
                        }
                    }
                    Ok::<_, GatewayError>(ContextRelDependencyBinding {
                        owner,
                        mount,
                        generation: session.rel_dependencies().generation,
                        memory_version,
                        synthesis_revision,
                    })
                })??;
                bindings.push(binding);
            }
            Ok(bindings)
        })
    }
}

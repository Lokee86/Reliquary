use crate::GeneralDecisionEndpoint;
use crate::{
    ConfiguredDecisionEndpoint, DecisionEndpoint, DecisionEndpointError, DecisionModelEndpoint,
    GeneralEndpoint, GeneralModelEndpoint, ModelSwitchboard, ModelSwitchboardConfig,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionConsumer {
    EntityResolution,
    MemoryRetrieval,
    Chronos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionRouteSelection<'a> {
    Dedicated(&'a DecisionModelEndpoint),
    Shared(&'a DecisionModelEndpoint),
    General(&'a GeneralModelEndpoint),
    Unavailable,
}

pub fn select_route<'a>(
    config: &'a ModelSwitchboardConfig,
    consumer: DecisionConsumer,
) -> DecisionRouteSelection<'a> {
    let operation_specific = match consumer {
        DecisionConsumer::EntityResolution => config.entity_resolution_decision.as_ref(),
        DecisionConsumer::MemoryRetrieval => config.memory_retrieval_decision.as_ref(),
        DecisionConsumer::Chronos => None,
    };
    if let Some(endpoint) = operation_specific {
        return DecisionRouteSelection::Dedicated(endpoint);
    }
    if let Some(endpoint) = config.general_decision.as_ref() {
        return DecisionRouteSelection::Shared(endpoint);
    }
    if let Some(endpoint) = config.general.as_ref() {
        return DecisionRouteSelection::General(endpoint);
    }
    DecisionRouteSelection::Unavailable
}

pub fn construct_decision_service(
    switchboard: &ModelSwitchboard,
    selected: DecisionRouteSelection<'_>,
    consumer: DecisionConsumer,
) -> Result<Option<ConfiguredDecisionEndpoint>, DecisionEndpointError> {
    let (endpoint, auth, route) = match selected {
        DecisionRouteSelection::Dedicated(endpoint) => {
            let (auth, route) = match consumer {
                DecisionConsumer::EntityResolution => (
                    switchboard.entity_resolution_decision_auth(),
                    "Entity resolution decision route is not configured",
                ),
                DecisionConsumer::MemoryRetrieval => (
                    switchboard.memory_retrieval_decision_auth(),
                    "Memory retrieval decision route is not configured",
                ),
                DecisionConsumer::Chronos => {
                    return Err(DecisionEndpointError::InvalidConfiguration(
                        "Chronos has no dedicated route",
                    ));
                }
            };
            (Some(endpoint), auth, route)
        }
        DecisionRouteSelection::Shared(endpoint) => (
            Some(endpoint),
            switchboard.general_decision_auth(),
            "Shared decision route is not configured",
        ),
        DecisionRouteSelection::General(_) | DecisionRouteSelection::Unavailable => {
            return Ok(None);
        }
    };
    ConfiguredDecisionEndpoint::from_route(endpoint, auth, route).map(Some)
}

pub fn construct_decision_endpoint(
    switchboard: &ModelSwitchboard,
    selected: DecisionRouteSelection<'_>,
    consumer: DecisionConsumer,
    general: Option<Arc<dyn GeneralEndpoint>>,
) -> Result<Option<Arc<dyn DecisionEndpoint>>, DecisionEndpointError> {
    match selected {
        DecisionRouteSelection::General(_) => {
            let general = general.ok_or(DecisionEndpointError::InvalidConfiguration(
                "general decision route requires a configured general endpoint",
            ))?;
            Ok(Some(Arc::new(GeneralDecisionEndpoint::new(general))))
        }
        DecisionRouteSelection::Unavailable => Ok(None),
        DecisionRouteSelection::Dedicated(_) | DecisionRouteSelection::Shared(_) => {
            construct_decision_service(switchboard, selected, consumer).map(|service| {
                service.map(|service| Arc::new(service) as Arc<dyn DecisionEndpoint>)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DecisionConsumer, DecisionRouteSelection, construct_decision_service, select_route,
    };
    use crate::{
        CredentialId, CredentialsConfig, DecisionEndpoint, DecisionModelEndpoint,
        GeneralModelEndpoint, ModelProvider, ModelSwitchboard, ModelSwitchboardConfig,
    };
    use serde_json::json;
    use std::io::{Read, Write};

    fn decision(model: &str) -> DecisionModelEndpoint {
        DecisionModelEndpoint {
            model: model.into(),
            url: format!("https://example.test/{model}"),
            credential_id: CredentialId::new(model).unwrap(),
        }
    }

    fn general(model: &str) -> GeneralModelEndpoint {
        GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: model.into(),
            url: Some("https://example.test/chat".into()),
            credential_id: CredentialId::new(model).unwrap(),
            reasoning_effort: None,
        }
    }

    fn selected_kind(config: &ModelSwitchboardConfig, consumer: DecisionConsumer) -> &'static str {
        match select_route(config, consumer) {
            DecisionRouteSelection::Dedicated(_) => "dedicated",
            DecisionRouteSelection::Shared(_) => "shared",
            DecisionRouteSelection::General(_) => "general",
            DecisionRouteSelection::Unavailable => "unavailable",
        }
    }

    #[test]
    fn precedence_is_operation_specific_then_shared_then_general_for_both_consumers() {
        for consumer in [
            DecisionConsumer::EntityResolution,
            DecisionConsumer::MemoryRetrieval,
        ] {
            for mask in 0_u8..8 {
                let mut config = ModelSwitchboardConfig::default();
                config.general = (mask & 1 != 0).then(|| general("general"));
                config.general_decision = (mask & 2 != 0).then(|| decision("shared"));
                match consumer {
                    DecisionConsumer::EntityResolution => {
                        config.entity_resolution_decision =
                            (mask & 4 != 0).then(|| decision("entity"));
                    }
                    DecisionConsumer::MemoryRetrieval => {
                        config.memory_retrieval_decision =
                            (mask & 4 != 0).then(|| decision("memory"));
                    }
                    DecisionConsumer::Chronos => unreachable!(),
                }
                let expected = if mask & 4 != 0 {
                    "dedicated"
                } else if mask & 2 != 0 {
                    "shared"
                } else if mask & 1 != 0 {
                    "general"
                } else {
                    "unavailable"
                };
                assert_eq!(selected_kind(&config, consumer), expected, "mask={mask}");
            }
        }
    }

    #[test]
    fn chronos_uses_shared_then_general_then_unavailable() {
        let mut config = ModelSwitchboardConfig::default();
        assert_eq!(
            selected_kind(&config, DecisionConsumer::Chronos),
            "unavailable"
        );
        config.general = Some(general("general"));
        assert_eq!(selected_kind(&config, DecisionConsumer::Chronos), "general");
        config.general_decision = Some(decision("shared"));
        assert_eq!(selected_kind(&config, DecisionConsumer::Chronos), "shared");
    }

    #[test]
    fn mixed_overrides_keep_each_consumers_own_specialized_route() {
        let mut general_endpoint = general("general");
        general_endpoint.credential_id = CredentialId::new("shared").unwrap();
        let config = ModelSwitchboardConfig {
            general: Some(general_endpoint),
            general_decision: Some(decision("shared")),
            entity_resolution_decision: Some(decision("entity")),
            memory_retrieval_decision: None,
            ..Default::default()
        };
        assert!(matches!(
            select_route(&config, DecisionConsumer::EntityResolution),
            DecisionRouteSelection::Dedicated(endpoint) if endpoint.model == "entity"
        ));
        assert!(matches!(
            select_route(&config, DecisionConsumer::MemoryRetrieval),
            DecisionRouteSelection::Shared(endpoint) if endpoint.model == "shared"
        ));
    }

    #[test]
    fn selected_shared_route_constructs_existing_decision_transport() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/selected", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut block = [0_u8; 2048];
                let read = stream.read(&mut block).unwrap();
                assert!(read > 0);
                bytes.extend_from_slice(&block[..read]);
                if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            assert!(headers.starts_with("POST /selected "));
            assert!(
                headers
                    .lines()
                    .any(|line| line.eq_ignore_ascii_case("authorization: Bearer shared-key"))
            );
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|value| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while bytes.len() < header_end + content_length {
                let mut block = [0_u8; 2048];
                let read = stream.read(&mut block).unwrap();
                assert!(read > 0);
                bytes.extend_from_slice(&block[..read]);
            }
            let request: serde_json::Value =
                serde_json::from_slice(&bytes[header_end..header_end + content_length]).unwrap();
            assert_eq!(request["model"], "shared");
            assert_eq!(request["state"], json!({"item": 7}));
            assert_eq!(request["questions"], json!({"choices": ["a", "b"]}));
            let body = b"{\"ok\":true}";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });

        let mut credentials = CredentialsConfig::default();
        credentials
            .insert_api_key(CredentialId::new("shared").unwrap(), "shared-key")
            .unwrap();
        credentials
            .insert_api_key(CredentialId::new("general").unwrap(), "general-key")
            .unwrap();
        let general_endpoint = general("general");
        let config = ModelSwitchboardConfig {
            general: Some(general_endpoint),
            general_decision: Some(DecisionModelEndpoint {
                model: "shared".into(),
                url,
                credential_id: CredentialId::new("shared").unwrap(),
            }),
            ..Default::default()
        };
        let switchboard = ModelSwitchboard::new(config, credentials).unwrap();
        let consumer = DecisionConsumer::EntityResolution;
        let selected = select_route(switchboard.config(), consumer);
        let endpoint = construct_decision_service(&switchboard, selected, consumer)
            .unwrap()
            .unwrap();
        assert_eq!(endpoint.model(), "shared");
        assert_eq!(
            endpoint
                .evaluate(&json!({"item": 7}), &json!({"choices": ["a", "b"]}))
                .unwrap(),
            json!({"ok": true})
        );
        server.join().unwrap();
    }

    #[test]
    fn selector_construction_rejects_invalid_routes_and_missing_credentials() {
        let mut credentials = CredentialsConfig::default();
        credentials
            .insert_api_key(CredentialId::new("entity").unwrap(), "entity-key")
            .unwrap();
        credentials
            .insert_api_key(CredentialId::new("shared").unwrap(), "shared-key")
            .unwrap();
        credentials
            .insert_api_key(CredentialId::new("general").unwrap(), "general-key")
            .unwrap();
        let config = ModelSwitchboardConfig {
            general: Some(general("general")),
            general_decision: Some(decision("shared")),
            entity_resolution_decision: Some(decision("entity")),
            ..Default::default()
        };
        let switchboard = ModelSwitchboard::new(config.clone(), credentials.clone()).unwrap();
        let entity_consumer = DecisionConsumer::EntityResolution;
        let entity_selection = select_route(switchboard.config(), entity_consumer);
        assert_eq!(
            construct_decision_service(&switchboard, entity_selection, entity_consumer)
                .unwrap()
                .unwrap()
                .model(),
            "entity"
        );
        let memory_consumer = DecisionConsumer::MemoryRetrieval;
        let memory_selection = select_route(switchboard.config(), memory_consumer);
        assert_eq!(
            construct_decision_service(&switchboard, memory_selection, memory_consumer)
                .unwrap()
                .unwrap()
                .model(),
            "shared"
        );

        let mut missing_credentials = CredentialsConfig::default();
        missing_credentials
            .insert_api_key(CredentialId::new("entity").unwrap(), "entity-key")
            .unwrap();
        assert!(ModelSwitchboard::new(config.clone(), missing_credentials).is_err());

        let invalid_config = ModelSwitchboardConfig {
            general_decision: Some(DecisionModelEndpoint {
                url: "https://".into(),
                ..decision("shared")
            }),
            ..config
        };
        assert!(ModelSwitchboard::new(invalid_config, credentials).is_err());
    }
}

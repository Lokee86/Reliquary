use super::{Instance, OwnerId, SharedEndpoint, now_ns};
use crate::{GatewayError, RelInsomniaClaimInput};
use reliquary_application::{
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, NativeInsomniaExtraction,
    commit_prepared_native_import_with_routed_receipts, extract_native_insomnia_with_ownership,
    resolve_import_insomnia_evidence, resolve_native_insomnia_evidence,
};
use reliquary_switchboard::GeneralEndpoint;

pub(crate) fn process_claim(
    instance: &Instance,
    owner: OwnerId,
    endpoint: &SharedEndpoint,
    phy: Option<OwnerId>,
    mut claim: RelInsomniaClaimInput,
    import_id: Option<&str>,
    ownership: Option<&SharedEndpoint>,
) -> Result<bool, GatewayError> {
    let destination = phy.map(OwnerId::as_bytes);
    let hold = |claim: &RelInsomniaClaimInput, reason: &str, payload: Option<Vec<u8>>| {
        if let Some(import_id) = import_id {
            instance.with_rel_writer(owner, |writer| {
                writer.hold_native_import_claim(import_id, claim, now_ns(), reason.into(), payload)
            })??;
        } else {
            instance.hold_insomnia_claim_with_payload(
                owner,
                claim,
                now_ns(),
                reason.into(),
                payload,
            )?;
        }
        Ok(false)
    };
    let extraction = match claim.work.held_payload.as_deref() {
        Some(payload) if import_id.is_some() => {
            NativeInsomniaExtraction::reopen_import_checkpoint(&claim, destination, payload)
        }
        Some(payload) => NativeInsomniaExtraction::reopen_checkpoint(&claim, destination, payload),
        None => extract_native_insomnia_with_ownership(
            endpoint.clone(),
            &claim,
            destination,
            (import_id.is_some() || destination.is_some())
                .then(|| ownership.unwrap_or(endpoint).clone()),
            |requests| {
                if let Some(id) = import_id {
                    instance
                        .with_rel_writer(owner, |writer| {
                            writer
                                .validate_native_import_claim(id, &claim, now_ns())
                                .map_err(|_| {
                                    "import evidence claim is no longer valid".to_owned()
                                })?;
                            resolve_import_insomnia_evidence(
                                writer.read_session(),
                                id,
                                &claim,
                                requests,
                            )
                        })
                        .map_err(|error| error.to_string())?
                } else {
                    instance
                        .with_writable_reliquary(owner, |rel| {
                            resolve_native_insomnia_evidence(rel, &claim, requests)
                        })
                        .map_err(|error| error.to_string())?
                }
            },
        ),
    };
    let extraction = match extraction {
        Ok(value) => value,
        Err(_) => {
            return hold(
                &claim,
                "Insomnia extraction failed validation or provider execution",
                claim.work.held_payload.clone(),
            );
        }
    };
    if import_id.is_some() && extraction.model() != endpoint.model() {
        return hold(
            &claim,
            "Insomnia checkpoint uses a different configured model",
            claim.work.held_payload.clone(),
        );
    }
    let payload = match extraction.checkpoint(&claim) {
        Ok(payload) => payload,
        Err(_) => return hold(&claim, "Insomnia checkpoint exceeds its bounds", None),
    };
    let model = extraction.model().to_owned();
    let prepared =
        match instance.with_writable_reliquary(owner, |rel| extraction.prepare(rel, &claim)) {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(_)) => return hold(&claim, "Insomnia source validation failed", Some(payload)),
            Err(error) => return Err(error),
        };
    if !prepared.rejected().is_empty() && import_id.is_none() {
        return hold(
            &claim,
            "Insomnia returned rejected candidates",
            Some(payload),
        );
    }
    let needs_phy = prepared.user_memories().next().is_some();
    if needs_phy && phy.is_none() {
        return hold(
            &claim,
            "Insomnia requires an authorized PHY destination",
            Some(payload),
        );
    }
    // The exact extraction survives an expired lease or process interruption.
    let checkpoint = match import_id {
        Some(id) => instance
            .with_rel_writer(owner, |writer| {
                writer.checkpoint_native_import_claim(id, &claim, now_ns(), payload.clone())
            })?
            .map_err(GatewayError::from),
        None => instance.checkpoint_insomnia_claim(owner, &claim, now_ns(), payload.clone()),
    };
    claim.work = match checkpoint {
        Ok(work) => work,
        Err(error) => {
            if hold(
                &claim,
                "Insomnia claim expired before publication",
                Some(payload),
            )
            .is_ok()
            {
                return Ok(false);
            }
            return Err(error);
        }
    };
    let published = publish(
        instance,
        owner,
        phy.filter(|_| needs_phy),
        &claim,
        import_id,
        &prepared,
        &model,
    );
    if let Err(error) = published {
        // A routed write may already have reached PHY. Keep the exact saved
        // proposal so an operator can retry its idempotent publication.
        if hold(
            &claim,
            "Insomnia publication requires reconciliation",
            claim.work.held_payload.clone(),
        )
        .is_ok()
        {
            return Ok(false);
        }
        return Err(error);
    }
    Ok(true)
}

fn publish(
    instance: &Instance,
    owner: OwnerId,
    phy: Option<OwnerId>,
    claim: &RelInsomniaClaimInput,
    import_id: Option<&str>,
    prepared: &reliquary_application::PreparedIngest,
    model: &str,
) -> Result<(), GatewayError> {
    let version = INSOMNIA_EXTRACTOR_CONTRACT_VERSION;
    if let Some(phy) = phy {
        instance.commit_scoped_routed_insomnia_claim(
            owner, phy, claim, prepared, model, version, now_ns, import_id,
        )?;
    } else if let Some(id) = import_id {
        instance.with_rel_writer(owner, |writer| {
            let now = now_ns();
            writer.validate_native_import_claim(id, claim, now)?;
            commit_prepared_native_import_with_routed_receipts(
                writer,
                id,
                prepared,
                &claim.work,
                model,
                version,
                now,
                &[],
            )
            .map_err(|error| GatewayError::RoutedMemory(error.to_string()))
        })??;
    } else {
        instance.commit_prepared_insomnia_claim(
            owner,
            claim,
            prepared,
            model,
            version,
            now_ns(),
        )?;
    }
    Ok(())
}

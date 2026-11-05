use super::{InsomniaWorkerConfig, Instance, OwnerId, SharedEndpoint, now_ns};
use crate::RelInsomniaClaimInput;
use reliquary_application::{
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, NativeInsomniaExtraction, extract_native_insomnia,
    resolve_native_insomnia_evidence,
};

pub(super) fn process_claim(
    instance: &Instance,
    owner: OwnerId,
    endpoint: &SharedEndpoint,
    config: &InsomniaWorkerConfig,
    mut claim: RelInsomniaClaimInput,
) {
    let destination = config.destination_phy_owner.map(OwnerId::as_bytes);
    let extraction = match claim.work.held_payload.as_deref() {
        Some(payload) => NativeInsomniaExtraction::reopen_checkpoint(&claim, destination, payload),
        None => extract_native_insomnia(endpoint.clone(), &claim, destination, |requests| {
            instance
                .with_writable_reliquary(owner, |rel| {
                    resolve_native_insomnia_evidence(rel, &claim, requests)
                })
                .map_err(|error| error.to_string())?
        }),
    };
    let extraction = match extraction {
        Ok(value) => value,
        Err(error) => {
            hold(
                instance,
                owner,
                &claim,
                &error,
                claim.work.held_payload.clone(),
            );
            return;
        }
    };
    let payload = match extraction.checkpoint(&claim) {
        Ok(payload) => payload,
        Err(error) => {
            hold(instance, owner, &claim, &error, None);
            return;
        }
    };
    let model = extraction.model().to_owned();
    let prepared =
        match instance.with_writable_reliquary(owner, |rel| extraction.prepare(rel, &claim)) {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                hold(instance, owner, &claim, &error.to_string(), Some(payload));
                return;
            }
            Err(_) => return,
        };
    if !prepared.rejected().is_empty() {
        hold(
            instance,
            owner,
            &claim,
            "Insomnia returned rejected candidates",
            Some(payload),
        );
        return;
    }
    let needs_phy = prepared.user_memories().next().is_some();
    if needs_phy && config.destination_phy_owner.is_none() {
        hold(
            instance,
            owner,
            &claim,
            "Insomnia requires an authorized PHY destination",
            Some(payload),
        );
        return;
    }
    // The exact extraction survives an expired lease or process interruption.
    claim.work = match instance.checkpoint_insomnia_claim(owner, &claim, now_ns(), payload) {
        Ok(work) => work,
        Err(_) => return,
    };
    let version = INSOMNIA_EXTRACTOR_CONTRACT_VERSION;
    if let Some(phy) = config.destination_phy_owner.filter(|_| needs_phy) {
        let _ = instance.commit_routed_insomnia_claim(
            owner,
            phy,
            &claim,
            &prepared,
            &model,
            version,
            now_ns(),
        );
    } else {
        let _ = instance.commit_prepared_insomnia_claim(
            owner,
            &claim,
            &prepared,
            &model,
            version,
            now_ns(),
        );
    }
}

fn hold(
    instance: &Instance,
    owner: OwnerId,
    claim: &RelInsomniaClaimInput,
    reason: &str,
    payload: Option<Vec<u8>>,
) {
    let reason = bounded_reason(reason.to_owned());
    let _ = instance.hold_insomnia_claim_with_payload(owner, claim, now_ns(), reason, payload);
}
fn bounded_reason(reason: String) -> String {
    reason.chars().take(2048).collect()
}

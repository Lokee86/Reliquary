use crate::{
    IngestError, PreparedIngest, ResolvedNativeInsomniaEvidence, prepare_native_ingest_from_archive,
};
use reliquary_insomnia::{
    EvidenceRequest, EvidenceTurn, InsomniaExtraction, InsomniaModelExtractor,
};
use reliquary_owner_session::{RelInsomniaClaimInput, RelSession};
use reliquary_switchboard::GeneralEndpoint;
use serde_json::{Value, json};

pub use reliquary_insomnia::INSOMNIA_EXTRACTOR_CONTRACT_VERSION;

pub struct NativeInsomniaExtraction {
    extraction: InsomniaExtraction,
    evidence: Vec<EvidenceTurn>,
    model: String,
    destination: Option<[u8; 16]>,
}

pub fn extract_native_insomnia<E: GeneralEndpoint + Clone + 'static>(
    endpoint: E,
    claim: &RelInsomniaClaimInput,
    destination: Option<[u8; 16]>,
    resolve: impl FnOnce(&[EvidenceRequest]) -> Result<ResolvedNativeInsomniaEvidence, String>,
) -> Result<NativeInsomniaExtraction, String> {
    let model = endpoint.model().to_owned();
    let mut extractor = InsomniaModelExtractor::new(endpoint.clone());
    if destination.is_some() {
        extractor = extractor.with_ownership_endpoint(endpoint);
    }
    let stage = extractor.start(&claim.episode, &claim.turns)?;
    let (extraction, evidence) = if stage.requests.is_empty() {
        (
            extractor.finish_without_evidence(&claim.episode, &claim.turns, stage)?,
            Vec::new(),
        )
    } else {
        let ResolvedNativeInsomniaEvidence {
            results,
            turns: evidence,
        } = resolve(&stage.requests)?;
        let extraction =
            extractor.finish_evidence(&claim.episode, &claim.turns, stage, &results, &evidence)?;
        (extraction, evidence)
    };
    Ok(NativeInsomniaExtraction {
        extraction,
        evidence,
        model,
        destination,
    })
}

impl NativeInsomniaExtraction {
    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn checkpoint(&self, claim: &RelInsomniaClaimInput) -> Result<Vec<u8>, String> {
        let payload = serde_json::to_vec(&json!({
            "version": 1,
            "owner": claim.owner_id,
            "episode": claim.episode.id.0,
            "model": self.model,
            "destination": self.destination,
            "extraction": self.extraction,
            "evidence": self.evidence,
        }))
        .map_err(|error| error.to_string())?;
        if payload.len() > reliquary_owner_session::MAX_INSOMNIA_CHECKPOINT_BYTES {
            return Err("Insomnia extraction exceeds durable checkpoint limit".into());
        }
        Ok(payload)
    }

    pub fn reopen_checkpoint(
        claim: &RelInsomniaClaimInput,
        destination: Option<[u8; 16]>,
        payload: &[u8],
    ) -> Result<Self, String> {
        let mut value: Value =
            serde_json::from_slice(payload).map_err(|error| error.to_string())?;
        if value["version"] != 1
            || value["owner"] != json!(claim.owner_id)
            || value["episode"] != json!(claim.episode.id.0)
            || value.get("destination") != Some(&json!(destination))
        {
            return Err("Insomnia checkpoint does not match claimed Episode".into());
        }
        let model = value["model"]
            .as_str()
            .filter(|model| !model.trim().is_empty())
            .ok_or("Insomnia checkpoint lacks its model")?
            .to_owned();
        let extraction = serde_json::from_value(value["extraction"].take())
            .map_err(|error| error.to_string())?;
        let evidence =
            serde_json::from_value(value["evidence"].take()).map_err(|error| error.to_string())?;
        Ok(Self {
            extraction,
            evidence,
            model,
            destination,
        })
    }

    pub fn prepare(
        self,
        rel: &RelSession,
        claim: &RelInsomniaClaimInput,
    ) -> Result<PreparedIngest, IngestError> {
        if rel.owner_id() != claim.owner_id || rel.episode(claim.episode.id) != Some(&claim.episode)
        {
            return Err(IngestError::EpisodeUnavailable);
        }
        let mut prepared = prepare_native_ingest_from_archive(
            rel,
            claim.episode.id,
            &self.evidence,
            crate::raw_insomnia_candidates(self.extraction.candidates),
        )?;
        prepared.claim_snapshot = Some((claim.episode.clone(), claim.turns.clone()));
        prepared.rejected.extend(self.extraction.rejected);
        Ok(prepared)
    }
}

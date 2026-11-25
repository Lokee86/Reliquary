//! Preparation and exact replay of owner-grouped first-cycle effects.
use super::*;
impl FreshnessStore {
    /// Prepare first-cycle enrollment without writing. Existing enrollment is
    /// retained; completing Dream never reanchors birth or admission.
    pub fn prepare_initial_admission(
        &self,
        id: FreshnessMemoryId,
        birth: u64,
        accepted: u64,
    ) -> Result<Vec<Vec<u8>>, String> {
        self.prepare_initial_enrollment(id, birth, accepted, true)
            .map(|(payloads, _)| payloads)
    }
    fn prepare_initial_enrollment(
        &self,
        memory_id: FreshnessMemoryId,
        birth_turn: u64,
        accepted_turn: u64,
        admit: bool,
    ) -> Result<(Vec<Vec<u8>>, Option<PendingBirth>), String> {
        if !self.policy_persisted
            || birth_turn == 0
            || accepted_turn < birth_turn
            || self
                .births
                .get(&memory_id)
                .is_some_and(|(turn, _)| *turn > birth_turn)
            || self
                .admissions
                .get(&memory_id)
                .is_some_and(|turn| *turn > accepted_turn)
        {
            return Err("invalid first-cycle Freshness cut".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        let mut effects = Vec::new();
        let (admitted_at_birth, pending_birth) = match self.births.get(&memory_id) {
            Some((_, admitted)) => (*admitted, None),
            None => {
                let entry = JournalEntry::Birth {
                    memory_id,
                    accepted_turn: birth_turn,
                    admitted_at_birth: false,
                };
                self.validate_entry(&entry)?;
                effects.push(encode_entry(owner, &entry)?);
                (
                    false,
                    Some(PendingBirth {
                        memory_id,
                        accepted_turn: birth_turn,
                    }),
                )
            }
        };
        if admit && !admitted_at_birth && !self.admissions.contains_key(&memory_id) {
            let entry = JournalEntry::Admission {
                memory_id,
                accepted_turn,
            };
            self.validate_entry_with_birth(&entry, pending_birth)?;
            effects.push(encode_entry(owner, &entry)?);
        }
        Ok((effects, pending_birth))
    }

    /// Encode an Application-evaluated first-cycle event after ordinary store
    /// birth, admission and event validation. This method performs no writes.
    pub fn prepare_initial_dream(
        &self,
        id: FreshnessMemoryId,
        birth: u64,
        accepted: u64,
        graph: u64,
        community: Option<u64>,
        effects: BTreeMap<FreshnessMemoryId, i16>,
    ) -> Result<Vec<Vec<u8>>, String> {
        self.prepare_initial_dream_lifecycle(id, birth, accepted, graph, community, effects, true)
    }
    pub fn prepare_initial_dream_lifecycle(
        &self,
        memory_id: FreshnessMemoryId,
        birth_turn: u64,
        accepted_turn: u64,
        graph_version: u64,
        community_generation: Option<u64>,
        effects: BTreeMap<FreshnessMemoryId, i16>,
        admit: bool,
    ) -> Result<Vec<Vec<u8>>, String> {
        let (mut payloads, pending_birth) =
            self.prepare_initial_enrollment(memory_id, birth_turn, accepted_turn, admit)?;
        if !effects.is_empty() {
            let owner = self.owner_uuid.ok_or("Freshness owner missing")?;
            let event_id = format!("dream-initial:{}:{}", hex(&owner), hex(&memory_id.0));
            let event = FreshnessEvent {
                event_id: event_id.clone(),
                accepted_turn,
                provenance: FreshnessEventProvenance {
                    producer_kind: 1,
                    source_id: event_id,
                    origin_owner: owner,
                    accepted_turn,
                    policy_version: u64::from(FRESHNESS_POLICY_VERSION),
                },
                proof: FreshnessEventProof {
                    graph_version,
                    community_generation,
                    community_graph_version: community_generation.map(|_| graph_version),
                },
                effects,
            };
            let entry = JournalEntry::Event(event);
            let payload = encode_entry(owner, &entry)?;
            self.validate_entry_with_birth(&entry, pending_birth)?;
            payloads.push(payload);
        }
        Ok(payloads)
    }

    /// Validate the exact canonical owner-prepared effects at their physical cut.
    pub fn validate_initial_dream(
        &self,
        id: FreshnessMemoryId,
        birth: u64,
        accepted: u64,
        graph: u64,
        community: Option<u64>,
        payloads: &[Vec<u8>],
    ) -> Result<BTreeMap<FreshnessMemoryId, i16>, String> {
        self.validate_initial_dream_lifecycle(id, birth, accepted, graph, community, payloads, true)
    }
    pub fn validate_initial_dream_lifecycle(
        &self,
        memory_id: FreshnessMemoryId,
        birth_turn: u64,
        accepted_turn: u64,
        graph_version: u64,
        community_generation: Option<u64>,
        payloads: &[Vec<u8>],
        admit: bool,
    ) -> Result<BTreeMap<FreshnessMemoryId, i16>, String> {
        let effects = Self::initial_dream_effects(payloads)?;
        if self.prepare_initial_dream_lifecycle(
            memory_id,
            birth_turn,
            accepted_turn,
            graph_version,
            community_generation,
            effects.clone(),
            admit,
        )? != payloads
        {
            return Err("Dream Freshness effects differ from accepted cut".into());
        }
        Ok(effects)
    }

    /// Selected typed effects for owner validation; this never admits a frame.
    pub fn initial_dream_effects(
        payloads: &[Vec<u8>],
    ) -> Result<BTreeMap<FreshnessMemoryId, i16>, String> {
        let mut effects = BTreeMap::new();
        for payload in payloads {
            if let Some((_, JournalEntry::Event(event))) = decode_entry(payload)? {
                if !effects.is_empty() {
                    return Err("multiple initial Dream events".into());
                }
                effects = event.effects;
            }
        }
        Ok(effects)
    }
}

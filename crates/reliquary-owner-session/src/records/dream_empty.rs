//! Atomic first-cycle acceptance, retained initial roots and settlement,
//! including owner-prepared Freshness effects.
use crate::MemoryId;
use reliquary_memory_core::MemoryBodyId;
use std::collections::BTreeSet;

const MAGIC: &[u8; 8] = b"RELDRF02";
const PREFIX: &[u8; 6] = b"RELDRF";
const MAX_PASS_BYTES: usize = 256;
const FIXED: usize = 8 + 16 + 2 + 32 + 8 + 32 + 8 + 8 + 2 + 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InitialDreamSettlement {
    pub owner: [u8; 16],
    pub pass_id: String,
    pub source: MemoryId,
    pub revision: u64,
    pub body: MemoryBodyId,
    pub graph_version: u64,
    pub accepted_turn: u64,
    pub community_generation: Option<u64>,
    pub roots: BTreeSet<MemoryId>,
    pub credit_roots: BTreeSet<MemoryId>,
    pub admit: bool,
    pub freshness_effects: Vec<Vec<u8>>,
    pub memory_delta: Option<Vec<u8>>,
}

impl InitialDreamSettlement {
    pub(crate) fn promotion_mutation(source: MemoryId) -> String {
        format!(
            "dream-initial-promotion:{}",
            source
                .0
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
    }
    pub(crate) fn memory_event(
        &self,
    ) -> Option<reliquary_memory_core::GroupedMemoryInitialPromotion> {
        self.memory_delta.clone().map(|delta| {
            reliquary_memory_core::GroupedMemoryInitialPromotion {
                source: self.source,
                previous_revision: self.revision,
                mutation_id: Self::promotion_mutation(self.source),
                delta,
            }
        })
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.credit_roots.len() > 16
            || self.credit_roots.contains(&self.source)
            || (!self.admit && self.memory_delta.is_some())
            || self.pass_id.is_empty()
            || self.pass_id.trim() != self.pass_id
            || self.pass_id.len() > MAX_PASS_BYTES
            || self.revision == 0
            || self.freshness_effects.len() > 3
            || self.roots.len() > 16
            || self.roots.contains(&self.source)
            || self
                .memory_delta
                .as_ref()
                .is_some_and(|d| d.len() > 65535 || !d.starts_with(b"RELMDLT1"))
            || self
                .freshness_effects
                .iter()
                .any(|effect| effect.len() > 4096)
        {
            return Err("invalid initial Dream settlement identity".into());
        }
        Ok(())
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = Vec::with_capacity(FIXED + self.pass_id.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.owner);
        out.extend_from_slice(&(self.pass_id.len() as u16).to_le_bytes());
        out.extend_from_slice(self.pass_id.as_bytes());
        out.extend_from_slice(&self.source.0);
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.extend_from_slice(&self.body.0);
        out.extend_from_slice(&self.graph_version.to_le_bytes());
        out.extend_from_slice(&self.accepted_turn.to_le_bytes());
        match self.community_generation {
            Some(generation) => {
                out.push(1);
                out.extend_from_slice(&generation.to_le_bytes());
            }
            None => out.push(0),
        }
        out.push(self.roots.len() as u8);
        for root in &self.roots {
            out.extend_from_slice(&root.0);
        }
        out.push(u8::from(self.admit));
        out.push(self.credit_roots.len() as u8);
        for root in &self.credit_roots {
            out.extend_from_slice(&root.0);
        }
        out.push(self.freshness_effects.len() as u8);
        for effect in &self.freshness_effects {
            out.extend_from_slice(&(effect.len() as u16).to_le_bytes());
            out.extend_from_slice(effect);
        }
        let delta = self.memory_delta.as_deref().unwrap_or_default();
        out.extend_from_slice(&(delta.len() as u32).to_le_bytes());
        out.extend_from_slice(delta);
        Ok(out)
    }

    pub(crate) fn decode(payload: &[u8]) -> Result<Option<Self>, String> {
        if !payload.starts_with(PREFIX) {
            return Ok(None);
        }
        let legacy = payload.starts_with(b"RELDRF01");
        if payload.len() < FIXED || (!legacy && !payload.starts_with(MAGIC)) {
            return Err("invalid initial Dream carrier".into());
        }
        let mut at = 8;
        let owner = take::<16>(payload, &mut at);
        let len = u16::from_le_bytes(take::<2>(payload, &mut at)) as usize;
        if len == 0 || len > MAX_PASS_BYTES || payload.len() < FIXED + len {
            return Err("invalid initial Dream pass length".into());
        }
        let pass_id = std::str::from_utf8(&payload[at..at + len])
            .map_err(|_| "initial Dream pass ID is not UTF-8")?
            .to_owned();
        at += len;
        let mut result = Self {
            owner,
            pass_id,
            source: MemoryId(take::<32>(payload, &mut at)),
            revision: u64::from_le_bytes(take::<8>(payload, &mut at)),
            body: MemoryBodyId(take::<32>(payload, &mut at)),
            graph_version: u64::from_le_bytes(take::<8>(payload, &mut at)),
            accepted_turn: u64::from_le_bytes(take::<8>(payload, &mut at)),
            community_generation: None,
            roots: BTreeSet::new(),
            credit_roots: BTreeSet::new(),
            admit: true,
            freshness_effects: Vec::new(),
            memory_delta: None,
        };
        let community = payload[at];
        at += 1;
        if community == 1 {
            if payload.len() < at + 10 {
                return Err("truncated Dream community".into());
            }
            result.community_generation = Some(u64::from_le_bytes(take::<8>(payload, &mut at)));
        } else if community != 0 {
            return Err("invalid Dream community".into());
        }
        let roots = payload[at] as usize;
        at += 1;
        if roots > 16 || payload.len() < at + roots * 32 + 1 {
            return Err("truncated Dream roots".into());
        }
        let mut previous = None;
        for _ in 0..roots {
            let root = MemoryId(take::<32>(payload, &mut at));
            if previous.is_some_and(|id| id >= root) {
                return Err("unsorted Dream roots".into());
            }
            previous = Some(root);
            if !result.roots.insert(root) {
                return Err("duplicate Dream root".into());
            }
        }
        if legacy {
            result.credit_roots = result.roots.clone();
        } else {
            if payload.len() < at + 2 {
                return Err("truncated Dream credit proof".into());
            }
            result.admit = match payload[at] {
                0 => false,
                1 => true,
                _ => return Err("invalid Dream admission flag".into()),
            };
            at += 1;
            let credits = payload[at] as usize;
            at += 1;
            if credits > 16 || payload.len() < at + credits * 32 + 1 {
                return Err("truncated Dream credit roots".into());
            }
            let mut previous = None;
            for _ in 0..credits {
                let root = MemoryId(take::<32>(payload, &mut at));
                if previous.is_some_and(|id| id >= root) {
                    return Err("unordered Dream credit roots".into());
                }
                previous = Some(root);
                result.credit_roots.insert(root);
            }
        }
        let count = payload[at] as usize;
        at += 1;
        if count > 3 {
            return Err("too many Dream Freshness effects".into());
        }
        for _ in 0..count {
            if payload.len() < at + 2 {
                return Err("truncated Dream Freshness length".into());
            }
            let size = u16::from_le_bytes(take::<2>(payload, &mut at)) as usize;
            if size > 4096 || payload.len() < at + size {
                return Err("truncated Dream Freshness effect".into());
            }
            result
                .freshness_effects
                .push(payload[at..at + size].to_vec());
            at += size;
        }
        if payload.len() < at + 4 {
            return Err("truncated Dream lifecycle length".into());
        }
        let delta_len = u32::from_le_bytes(take::<4>(payload, &mut at)) as usize;
        if delta_len > 65535 || payload.len() < at + delta_len {
            return Err("truncated Dream lifecycle".into());
        }
        if delta_len > 0 {
            result.memory_delta = Some(payload[at..at + delta_len].to_vec());
        }
        at += delta_len;
        if at != payload.len() {
            return Err("trailing Dream settlement bytes".into());
        }
        result.validate()?;
        Ok(Some(result))
    }
}

fn take<const N: usize>(payload: &[u8], at: &mut usize) -> [u8; N] {
    let value = payload[*at..*at + N].try_into().expect("length verified");
    *at += N;
    value
}

#[cfg(test)]
#[path = "dream_empty_tests.rs"]
mod tests;

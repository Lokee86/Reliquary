//! Test-only canonical owner-state fingerprints. Never a durable manifest.
//!
//! Owners supply decoded semantic rows with stable keys. Physical locations,
//! codec bytes, derived indexes and Debug output must not enter those rows.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalFingerprint {
    pub(crate) schema: u32,
    pub(crate) owner_uuid: [u8; 16],
    pub(crate) domain: String,
    pub(crate) local_cut: u64,
    pub(crate) event_cut: Option<u64>,
    pub(crate) rows: u64,
    pub(crate) sha256: [u8; 32],
}

pub(crate) struct OwnerProjection {
    owner_uuid: [u8; 16],
    domain: String,
    local_cut: u64,
    event_cut: Option<u64>,
    rows: BTreeMap<(String, Vec<u8>), Value>,
}

impl OwnerProjection {
    pub(crate) fn new(
        owner_uuid: [u8; 16],
        domain: &str,
        local_cut: u64,
        event_cut: Option<u64>,
    ) -> Result<Self, String> {
        if domain.is_empty() {
            return Err("fingerprint domain must be explicit".into());
        }
        Ok(Self {
            owner_uuid,
            domain: domain.into(),
            local_cut,
            event_cut,
            rows: BTreeMap::new(),
        })
    }

    pub(crate) fn insert(
        &mut self,
        family: &str,
        key: impl Into<Vec<u8>>,
        value: Value,
    ) -> Result<(), String> {
        if family.is_empty() {
            return Err("fingerprint row family must be explicit".into());
        }
        let key = (family.to_owned(), key.into());
        if self.rows.contains_key(&key) {
            return Err("duplicate semantic fingerprint row".into());
        }
        // Validate recursively before changing even this disposable projection.
        canonical_value(&value)?;
        self.rows.insert(key, value);
        Ok(())
    }

    pub(crate) fn fingerprint(&self) -> Result<HistoricalFingerprint, String> {
        let mut hash = Sha256::new();
        hash.update(b"reliquary:historical-owner-state:v1\0");
        hash.update(1_u32.to_le_bytes());
        hash.update(self.owner_uuid);
        hash_field(&mut hash, self.domain.as_bytes());
        hash.update(self.local_cut.to_le_bytes());
        match self.event_cut {
            None => hash.update([0]),
            Some(cut) => {
                hash.update([1]);
                hash.update(cut.to_le_bytes());
            }
        }
        hash.update((self.rows.len() as u64).to_le_bytes());
        for ((family, key), value) in &self.rows {
            hash_field(&mut hash, family.as_bytes());
            hash_field(&mut hash, key);
            hash_field(&mut hash, &canonical_value(value)?);
        }
        Ok(HistoricalFingerprint {
            schema: 1,
            owner_uuid: self.owner_uuid,
            domain: self.domain.clone(),
            local_cut: self.local_cut,
            event_cut: self.event_cut,
            rows: self.rows.len() as u64,
            sha256: hash.finalize().into(),
        })
    }
}

fn hash_field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

// Explicit key sorting remains deterministic if dependency feature unification
// ever enables serde_json's insertion-ordered maps. Array order is semantic.
fn canonical_value(value: &Value) -> Result<Vec<u8>, String> {
    fn write(value: &Value, out: &mut Vec<u8>) -> Result<(), String> {
        match value {
            Value::Object(fields) => {
                out.push(b'{');
                let sorted: BTreeMap<_, _> = fields.iter().collect();
                for (index, (key, value)) in sorted.into_iter().enumerate() {
                    if index != 0 {
                        out.push(b',');
                    }
                    out.extend(serde_json::to_vec(key).map_err(|e| e.to_string())?);
                    out.push(b':');
                    write(value, out)?;
                }
                out.push(b'}');
            }
            Value::Array(values) => {
                out.push(b'[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        out.push(b',');
                    }
                    write(value, out)?;
                }
                out.push(b']');
            }
            Value::Number(number) if !number.is_i64() && !number.is_u64() => {
                return Err("encode semantic floating values as exact integer bits".into());
            }
            scalar => out.extend(serde_json::to_vec(scalar).map_err(|e| e.to_string())?),
        }
        Ok(())
    }
    let mut bytes = Vec::new();
    write(value, &mut bytes)?;
    Ok(bytes)
}

#[path = "historical_fingerprint_tests.rs"]
mod tests;

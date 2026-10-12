//! H1 typed canonical encoding. This is an ephemeral proof, never a second store.
use super::fail;
use crate::CvaError;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Value {
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Value>),
    Map(Vec<(String, Value)>),
}
impl Value {
    pub(super) fn bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self::Bytes(bytes.as_ref().to_vec())
    }
    pub(super) fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }
    pub(super) fn map<const N: usize>(fields: [(&str, Self); N]) -> Self {
        Self::Map(
            fields
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }
    pub(super) fn encoded(&self) -> Result<Vec<u8>, CvaError> {
        fn len(out: &mut Vec<u8>, count: usize) -> Result<(), CvaError> {
            out.extend(
                u32::try_from(count)
                    .map_err(|_| fail("historical canonical length overflow"))?
                    .to_le_bytes(),
            );
            Ok(())
        }
        fn encode(value: &Value, out: &mut Vec<u8>) -> Result<(), CvaError> {
            match value {
                Value::Null => out.push(0),
                Value::Bool(value) => out.extend([1, u8::from(*value)]),
                Value::Signed(value) => {
                    out.push(2);
                    out.extend(value.to_le_bytes());
                }
                Value::Unsigned(value) => {
                    out.push(3);
                    out.extend(value.to_le_bytes());
                }
                Value::Bytes(value) => {
                    out.push(4);
                    len(out, value.len())?;
                    out.extend(value);
                }
                Value::Text(value) => {
                    out.push(5);
                    len(out, value.len())?;
                    out.extend(value.as_bytes());
                }
                Value::Array(values) => {
                    out.push(6);
                    len(out, values.len())?;
                    for value in values {
                        encode(value, out)?;
                    }
                }
                Value::Map(fields) => {
                    let mut fields: Vec<_> = fields.iter().collect();
                    fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                    if fields.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                        return Err(fail("duplicate historical canonical key"));
                    }
                    out.push(7);
                    len(out, fields.len())?;
                    for (key, value) in fields {
                        encode(&Value::text(key), out)?;
                        encode(value, out)?;
                    }
                }
            }
            Ok(())
        }
        let mut out = Vec::new();
        encode(self, &mut out)?;
        Ok(out)
    }
    pub(super) fn digest(&self, domain: &str) -> Result<[u8; 32], CvaError> {
        if !domain.is_ascii() || domain.as_bytes().contains(&0) {
            return Err(fail("invalid historical hash domain"));
        }
        let mut hash = Sha256::new();
        hash.update(domain.as_bytes());
        hash.update([0]);
        hash.update(self.encoded()?);
        Ok(hash.finalize().into())
    }
}
pub(super) fn policy() -> Value {
    Value::map([
        ("version", Value::Unsigned(1)),
        (
            "timeline",
            Value::text("ready-timestamp-utf8-topological-v1"),
        ),
        ("clock", Value::text("exact-archive-prefix-v1")),
        ("availability", Value::text("latest-evidence-cohort-v1")),
        (
            "missing_provenance",
            Value::text("unsupported-preserve-or-reject-v1"),
        ),
        ("scope", Value::text("single-owner-rel-v1")),
        ("community", Value::text("validated-or-unknown-outside-v1")),
        ("semantic", Value::text("f8-v1-eligible-top64-max-v1")),
        (
            "dream",
            Value::text("actual-no-link-before-final-fallback-v1"),
        ),
        (
            "credit",
            Value::text("availability-inclusive-self-cohort-excluded-v1"),
        ),
        (
            "traversal",
            Value::text("authorized-full-graph-credit-filter-only-v1"),
        ),
        (
            "event_order",
            Value::text("admission-then-position-event-id-v1"),
        ),
        (
            "scoring",
            Value::text("existing-per-event-caps-lazy-tail-v1"),
        ),
        (
            "replacement",
            Value::text("provisional-isolated-subset-atomic-v1"),
        ),
        (
            "activation",
            Value::text("historical-before-prospective-live-v1"),
        ),
        ("encoding", Value::text("typed-le-c-v1")),
        ("identity", Value::text("owner-corpus-stable-import-v1")),
    ])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_h1_policy_golden_digest() {
        let actual: String = policy()
            .digest("reliquary:historical-policy:v1")
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            policy().digest("reliquary:historical-policy:v1").unwrap(),
            crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST
        );
        assert_eq!(
            actual,
            "0cad2e7fd763de37753a54c28df1c7b6303c6c818145492ca4a709023c420908"
        );
    }
    #[test]
    fn historical_canonical_encoding_golden_and_map_order() {
        let value = Value::Array(vec![
            Value::Null,
            Value::Bool(true),
            Value::Signed(-2),
            Value::Unsigned(3),
            Value::bytes([0xff]),
            Value::text("é"),
        ]);
        assert_eq!(
            value.encoded().unwrap(),
            vec![
                6, 6, 0, 0, 0, 0, 1, 1, 2, 254, 255, 255, 255, 255, 255, 255, 255, 3, 3, 0, 0, 0,
                0, 0, 0, 0, 4, 1, 0, 0, 0, 255, 5, 2, 0, 0, 0, 195, 169
            ]
        );
        let a = Value::map([("z", Value::Null), ("a", Value::Bool(false))]);
        let b = Value::map([("a", Value::Bool(false)), ("z", Value::Null)]);
        assert_eq!(a.encoded().unwrap(), b.encoded().unwrap());
        assert!(
            Value::map([("a", Value::Null), ("a", Value::Null)])
                .encoded()
                .is_err()
        );
    }
}

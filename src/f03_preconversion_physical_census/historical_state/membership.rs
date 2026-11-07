//! Exact local-cut membership for retained, clocked native projections.
use super::row_commitment::RowCommitment;
use super::{ArtifactSink, BatchWriter, ToolResult, VerifiedCopy, lower_hex, sha256_bytes};
use crate::historical_fingerprint::{canonical_value, hash_field};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

type Digest32 = [u8; 32];
type Key = (&'static str, Digest32);
type Dependencies = BTreeMap<&'static str, BTreeMap<Digest32, u64>>;
type DomainRows<'a> = BTreeMap<&'a str, Vec<&'a RowCommitment>>;

#[derive(Default)]
pub(super) struct Anchor {
    cut: Option<u64>,
    subject: Option<Digest32>,
    revision: Option<u64>,
    dependencies: Vec<Key>,
}

fn identity(value: &Value) -> Option<Digest32> {
    let bytes: Vec<u8> = if let Some(text) = value.as_str() {
        if text.len() != 32 && text.len() != 64 {
            return None;
        }
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
            .collect::<Option<_>>()?
    } else {
        value
            .as_array()?
            .iter()
            .map(|element| u8::try_from(element.as_u64()?).ok())
            .collect::<Option<_>>()?
    };
    Some(Sha256::digest(bytes).into())
}

fn add_dependency(anchor: &mut Anchor, family: &'static str, id: &Value) {
    if let Some(digest) = identity(id) {
        anchor.dependencies.push((family, digest));
    }
}

pub(super) fn anchor(domain: &str, family: &str, key: &[u8], value: &Value) -> Anchor {
    let mut result = Anchor::default();
    result.cut = match (domain, family) {
        ("archive" | "archive_publication_origins", "publication" | "publication_origin")
        | ("graph" | "memory_graph" | "ego", "publication")
        | ("vector_generation", "generation") => {
            <[u8; 8]>::try_from(key).ok().map(u64::from_be_bytes)
        }
        ("graph" | "memory_graph", "baseline_memory") => Some(0),
        ("memory" | "entity" | "relationship", "revision" | "terminal") => {
            result.subject = identity(&value["id"]);
            result.revision = (family == "revision")
                .then(|| value["revision"].as_u64())
                .flatten();
            let field = match domain {
                "memory" => "memory_version",
                "entity" => "entity_version",
                _ => "relationship_version",
            };
            value[field].as_u64()
        }
        _ => None,
    };
    match (domain, family) {
        ("archive", "publication") => {
            let record = &value["record"];
            for node in [&record["node"], &record["file"], &record["turn"]["node"]] {
                add_dependency(&mut result, "content", &node["content_id"]);
            }
            if let Some(files) = record["turn"]["attachments"].as_array() {
                for file in files {
                    add_dependency(&mut result, "content", &file["content_id"]);
                }
            }
        }
        ("memory", "revision") => {
            add_dependency(&mut result, "body", &value["body_id"]);
            add_dependency(&mut result, "routing", &value["id"]);
            add_dependency(&mut result, "external_evidence", &value["id"]);
        }
        ("vector_generation", "generation") => {
            add_dependency(&mut result, "profile", &value["compatibility_profile_id"]);
            add_dependency(&mut result, "archive_vectors", &value["archive_vector_id"]);
        }
        ("vector_generation", "archive_vectors") => {
            add_dependency(&mut result, "packed_vectors", &value["packed_vector_id"]);
        }
        _ => {}
    }
    result
}

struct Interval<'a> {
    domain: &'a str,
    family: &'a str,
    key: Digest32,
    value: String,
    availability: &'a str,
    first: u64,
    last: u64,
}

impl Interval<'_> {
    fn membership(&self) -> Value {
        json!({
            "projection_domain": self.domain,
            "family": self.family,
            "stable_key_sha256": lower_hex(&self.key),
            "canonical_value_sha256": self.value,
            "availability": self.availability,
        })
    }
}

fn consider_dependencies(dependencies: &[Key], cut: u64, first: &mut Dependencies) {
    for &(family, key) in dependencies {
        first
            .entry(family)
            .or_default()
            .entry(key)
            .and_modify(|n| *n = (*n).min(cut))
            .or_insert(cut);
    }
}

fn intervals<'a>(
    domain: &'a str,
    head: u64,
    rows: &[&'a RowCommitment],
    memory_graph_positions: &[u64],
) -> ToolResult<Vec<Interval<'a>>> {
    let mut dependencies = Dependencies::new();
    let mut revisions = BTreeMap::<Digest32, Vec<(u64, Option<u64>)>>::new();
    for row in rows {
        if let Some(cut) = row.anchor.cut {
            if cut > head {
                return Err("retained row lies beyond its local head".into());
            }
            consider_dependencies(&row.anchor.dependencies, cut, &mut dependencies);
            if matches!(domain, "memory" | "entity" | "relationship") {
                let subject = row
                    .anchor
                    .subject
                    .ok_or("revision has no typed stable subject")?;
                revisions
                    .entry(subject)
                    .or_default()
                    .push((cut, row.anchor.revision));
            }
        }
    }
    if domain == "vector_generation" {
        for row in rows.iter().filter(|r| r.family == "archive_vectors") {
            if let Some(&cut) = dependencies
                .get("archive_vectors")
                .and_then(|rows| rows.get(&row.key_sha256))
            {
                consider_dependencies(&row.anchor.dependencies, cut, &mut dependencies);
            }
        }
    }

    let mut result = Vec::with_capacity(rows.len());
    let mut expected_head = BTreeMap::new();
    for row in rows {
        let cut = row.anchor.cut.or_else(|| {
            dependencies
                .get(row.family.as_str())
                .and_then(|keys| keys.get(&row.key_sha256))
                .copied()
        });
        expected_head.insert(
            (row.family.as_str(), row.key_sha256),
            lower_hex(&row.value_sha256),
        );
        if let Some(first) = cut {
            result.push(Interval {
                domain,
                family: &row.family,
                key: row.key_sha256,
                value: lower_hex(&row.value_sha256),
                availability: row.availability,
                first,
                last: head,
            });
        } else if !matches!(row.family.as_str(), "current" | "clock") {
            return Err(format!("no local-cut origin for {domain}/{}", row.family));
        }
    }
    for (key, mut events) in revisions {
        events.sort_unstable();
        if events.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err("duplicate current revision position".into());
        }
        for (index, &(first, revision)) in events.iter().enumerate() {
            if let Some(revision) = revision {
                result.push(Interval {
                    domain,
                    family: "current",
                    key,
                    value: sha256_bytes(revision.to_string().as_bytes()),
                    availability: "retained",
                    first,
                    last: events.get(index + 1).map_or(head, |next| next.0 - 1),
                });
            }
        }
    }
    if matches!(domain, "graph" | "memory_graph") {
        let key = Sha256::digest(b"memory_graph").into();
        for (index, &first) in memory_graph_positions.iter().enumerate() {
            if first <= head {
                result.push(Interval {
                    domain,
                    family: "clock",
                    key,
                    value: sha256_bytes(first.to_string().as_bytes()),
                    availability: "retained",
                    first,
                    last: memory_graph_positions
                        .get(index + 1)
                        .map_or(head, |next| next - 1)
                        .min(head),
                });
            }
        }
    }
    let observed_head: BTreeMap<_, _> = result
        .iter()
        .filter(|row| row.last == head)
        .map(|row| ((row.family, row.key), row.value.clone()))
        .collect();
    if expected_head != observed_head {
        return Err(format!(
            "{domain} membership does not reproduce the native retained head"
        ));
    }
    Ok(result)
}

pub(super) struct CutProofs {
    pub(super) metadata: Value,
    pub(super) roots: BTreeMap<String, Vec<String>>,
}

pub(super) fn emit(
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
    rows: &[RowCommitment],
    projections: &[Value],
) -> ToolResult<CutProofs> {
    let mut grouped = DomainRows::new();
    for row in rows {
        grouped.entry(&row.domain).or_default().push(row);
    }
    let memory_graph_positions: Vec<_> = std::iter::once(0)
        .chain(
            grouped
                .get("memory_graph")
                .into_iter()
                .flatten()
                .filter(|row| row.family == "publication")
                .filter_map(|row| row.anchor.cut)
                .collect::<BTreeSet<_>>(),
        )
        .collect();

    let mut stream = BatchWriter::new(sink, copy, "clocked-membership");
    let mut roots = BTreeMap::new();
    let mut covered = BTreeMap::new();
    for projection in projections {
        let domain = projection["domain"]
            .as_str()
            .ok_or("projection has no domain")?;
        if !matches!(
            domain,
            "archive"
                | "archive_publication_origins"
                | "memory"
                | "entity"
                | "relationship"
                | "graph"
                | "memory_graph"
                | "ego"
                | "vector_generation"
        ) {
            continue;
        }
        let head: u64 = projection["local_cut"]
            .as_str()
            .ok_or("projection has no head")?
            .parse()
            .map_err(|_| "invalid projection head")?;
        let domain_rows = grouped.get(domain).map_or(&[][..], Vec::as_slice);
        let mut members = intervals(domain, head, domain_rows, &memory_graph_positions)?;
        members.sort_unstable_by_key(|r| (r.first, r.family, r.key, r.last));
        let mut activations = BTreeMap::<u64, Vec<Vec<u8>>>::new();
        for row in &members {
            let membership = row.membership();
            let value = canonical_value(&membership)?;
            activations
                .entry(row.first)
                .or_default()
                .push([b"+".as_slice(), &value].concat());
            if row.last < head {
                activations
                    .entry(row.last + 1)
                    .or_default()
                    .push([b"-".as_slice(), &value].concat());
            }
            stream.push(json!({
                "row": membership,
                "first_cut": row.first.to_string(),
                "last_cut": row.last.to_string(),
            }))?;
        }
        let mut hash = Sha256::new();
        hash.update(b"reliquary:f03:retained-local-prefix:v1\0");
        hash_field(&mut hash, domain.as_bytes());
        let mut prefix = Vec::with_capacity((head + 1) as usize);
        for cut in 0..=head {
            if let Some(changes) = activations.get_mut(&cut) {
                changes.sort_unstable();
                for change in changes {
                    hash.update(cut.to_be_bytes());
                    hash_field(&mut hash, change);
                }
            }
            prefix.push(lower_hex(&hash.clone().finalize()));
        }
        roots.insert(domain.to_string(), prefix);
        covered.insert(domain.to_string(), json!({
            "head": head.to_string(),
            "cut_shape": if domain == "memory_graph" { "sparse_graph_publications" } else { "inclusive_dense" },
            "intervals": members.len().to_string(),
            "qualifying_cuts": (domain == "memory_graph").then(|| memory_graph_positions.iter().map(u64::to_string).collect::<Vec<_>>()),
        }));
    }
    Ok(CutProofs {
        metadata: json!({
            "scope": "retained_clocked_native_history",
            "stream": stream.finish()?,
            "domains": covered,
        }),
        roots,
    })
}

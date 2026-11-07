use reliquary_archive::{Branch, ContentId, Node};
use reliquary_owner_session::{HistoricalOwnerProof, OwnerScope, RelHistoricalCut, RelWriter};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    format: Format,
    owner: String,
    scope: Scope,
    history_generation: String,
    revisions: Vec<Revision>,
}

#[derive(Deserialize)]
enum Format {
    #[serde(rename = "reliquary-d01-synthetic-archive-transfer-v1")]
    Archive,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    Unspecified,
    Organization,
    Project,
    Connection,
}

impl From<Scope> for OwnerScope {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::Unspecified => Self::Unspecified,
            Scope::Organization => Self::Organization,
            Scope::Project => Self::Project,
            Scope::Connection => Self::Connection,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
enum Revision {
    Turn {
        version: String,
        conversation: String,
        node: String,
        #[serde(deserialize_with = "present_option")]
        parent: Option<String>,
        role: String,
        #[serde(deserialize_with = "present_option")]
        principal: Option<String>,
        timestamp_ns: String,
        content_id: String,
        content: String,
    },
    Branch {
        version: String,
        conversation: String,
        branch: String,
        leaf: String,
        canonical: bool,
    },
}

impl Transfer {
    pub fn owner(&self) -> Result<[u8; 16], &'static str> {
        decode_hex(&self.owner)
    }

    pub fn scope(&self) -> OwnerScope {
        self.scope.into()
    }

    pub fn generation(&self) -> Result<[u8; 32], &'static str> {
        decode_hex(&self.history_generation)
    }

    pub fn replay(
        self,
        writer: &mut RelWriter,
    ) -> Result<Vec<(RelHistoricalCut, HistoricalOwnerProof)>, &'static str> {
        let Format::Archive = self.format;
        if !(1..=64).contains(&self.revisions.len()) {
            return Err("synthetic Archive revision count is outside bounds");
        }
        let mut cuts = vec![proof(writer)?];
        for revision in self.revisions {
            match revision {
                Revision::Turn {
                    version,
                    conversation,
                    node,
                    parent,
                    role,
                    principal,
                    timestamp_ns,
                    content_id,
                    content,
                } => writer.import_accepted_archive_turn(
                    decimal(&version)?,
                    Node {
                        id: node,
                        conversation_id: conversation,
                        parent_id: parent,
                        role,
                        principal_id: principal,
                        timestamp_ns: decimal(&timestamp_ns)?,
                        content_id: ContentId(decode_hex(&content_id)?),
                    },
                    content,
                ),
                Revision::Branch {
                    version,
                    conversation,
                    branch,
                    leaf,
                    canonical,
                } => writer.import_accepted_archive_branch(
                    decimal(&version)?,
                    Branch {
                        id: branch,
                        conversation_id: conversation,
                        leaf_node_id: leaf,
                        canonical,
                    },
                ),
            }
            .map_err(|_| "accepted Archive revision refused by current owner")?;
            cuts.push(proof(writer)?);
        }
        Ok(cuts)
    }
}

fn proof(writer: &RelWriter) -> Result<(RelHistoricalCut, HistoricalOwnerProof), &'static str> {
    let reader = writer.read_session();
    let cut = reader.current_historical_cut();
    let proof = reader
        .historical_proof(reader.owner_id(), &cut)
        .map_err(|_| "written Archive cut did not produce a historical proof")?;
    Ok((cut, proof))
}

fn present_option<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Option<String>, D::Error> {
    Option::deserialize(input)
}

pub fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], &'static str> {
    if value.len() != N * 2 || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err("invalid lowercase hexadecimal field");
    }
    let mut bytes = [0; N];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| "invalid lowercase hexadecimal field")?;
    Ok(bytes)
}

fn decimal<T: std::str::FromStr + ToString>(value: &str) -> Result<T, &'static str> {
    let parsed: T = value.parse().map_err(|_| "invalid decimal field")?;
    if parsed.to_string() != value {
        return Err("noncanonical decimal field");
    }
    Ok(parsed)
}

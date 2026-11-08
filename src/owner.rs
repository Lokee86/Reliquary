use crate::{ProductError, ProductErrorKind};
use reliquary_owner_session::{PhySession, PhyWriter, RelSession, RelWriter};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnerId([u8; 16]);

impl OwnerId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}
impl fmt::Display for OwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
impl FromStr for OwnerId {
    type Err = ProductError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ProductError::invalid(
                "owner ID must contain 32 hexadecimal characters",
            ));
        }
        let mut bytes = [0u8; 16];
        for (index, slot) in bytes.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
                ProductError::invalid("owner ID contains non-hexadecimal characters")
            })?;
        }
        Ok(Self(bytes))
    }
}
impl Serialize for OwnerId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for OwnerId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}
impl From<OwnerId> for reliquary_runtime_gateway::OwnerId {
    fn from(value: OwnerId) -> Self {
        Self::from_bytes(value.0)
    }
}
impl From<reliquary_runtime_gateway::OwnerId> for OwnerId {
    fn from(value: reliquary_runtime_gateway::OwnerId) -> Self {
        Self(value.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerKind {
    Rel,
    Phylactery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerScope {
    Unspecified,
    Organization,
    Project,
    Connection,
}

impl From<OwnerScope> for reliquary_owner_session::OwnerScope {
    fn from(value: OwnerScope) -> Self {
        match value {
            OwnerScope::Unspecified => Self::Unspecified,
            OwnerScope::Organization => Self::Organization,
            OwnerScope::Project => Self::Project,
            OwnerScope::Connection => Self::Connection,
        }
    }
}
impl From<reliquary_owner_session::OwnerScope> for OwnerScope {
    fn from(value: reliquary_owner_session::OwnerScope) -> Self {
        match value {
            reliquary_owner_session::OwnerScope::Unspecified => Self::Unspecified,
            reliquary_owner_session::OwnerScope::Organization => Self::Organization,
            reliquary_owner_session::OwnerScope::Project => Self::Project,
            reliquary_owner_session::OwnerScope::Connection => Self::Connection,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OwnerInfo {
    pub format: &'static str,
    pub kind: OwnerKind,
    pub owner_id: OwnerId,
    pub scope: OwnerScope,
    pub memory_count: usize,
    pub memory_version: u64,
    pub graph_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel_turn_count: Option<u64>,
}

#[derive(Clone)]
pub struct Rel {
    path: Arc<PathBuf>,
    session: Arc<RelSession>,
}
impl Rel {
    pub fn create(path: impl AsRef<Path>, scope: OwnerScope) -> Result<Self, ProductError> {
        let writer = RelWriter::create_current_path(
            path.as_ref(),
            scope.into(),
            *Uuid::new_v4().as_bytes(),
        )?;
        drop(writer);
        Self::open(path)
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProductError> {
        let path = path.as_ref().to_path_buf();
        let session = RelSession::open_path_current(&path)?;
        Ok(Self {
            path: Arc::new(path),
            session: Arc::new(session),
        })
    }
    pub fn verify(path: impl AsRef<Path>) -> Result<OwnerInfo, ProductError> {
        Self::open(path)?.info()
    }
    pub fn owner_id(&self) -> OwnerId {
        OwnerId::from_bytes(self.session.owner_id())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn info(&self) -> Result<OwnerInfo, ProductError> {
        Ok(OwnerInfo {
            format: "2.1",
            kind: OwnerKind::Rel,
            owner_id: self.owner_id(),
            scope: self.session.scope().into(),
            memory_count: count_rel(&self.session)?,
            memory_version: self.session.memory_version(),
            graph_version: self.session.graph_version(),
            archive_version: Some(self.session.archive_version()),
            rel_turn_count: Some(self.session.rel_turn_count()),
        })
    }
    pub(crate) fn session(&self) -> &RelSession {
        &self.session
    }
}

#[derive(Clone)]
pub struct Phylactery {
    path: Arc<PathBuf>,
    session: Arc<PhySession>,
}
impl Phylactery {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, ProductError> {
        let writer = PhyWriter::create_path(path.as_ref(), *Uuid::new_v4().as_bytes())?;
        drop(writer);
        Self::open(path)
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProductError> {
        let path = path.as_ref().to_path_buf();
        let session = PhySession::open_path_current(&path)?;
        Ok(Self {
            path: Arc::new(path),
            session: Arc::new(session),
        })
    }
    pub fn verify(path: impl AsRef<Path>) -> Result<OwnerInfo, ProductError> {
        Self::open(path)?.info()
    }
    pub fn owner_id(&self) -> OwnerId {
        OwnerId::from_bytes(self.session.owner_id())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn info(&self) -> Result<OwnerInfo, ProductError> {
        Ok(OwnerInfo {
            format: "2.1",
            kind: OwnerKind::Phylactery,
            owner_id: self.owner_id(),
            scope: OwnerScope::Unspecified,
            memory_count: count_phy(&self.session)?,
            memory_version: self.session.memory_version(),
            graph_version: self.session.graph_version(),
            archive_version: None,
            rel_turn_count: None,
        })
    }
    pub(crate) fn session(&self) -> &PhySession {
        &self.session
    }
}

fn count_rel(session: &RelSession) -> Result<usize, ProductError> {
    let mut after = None;
    let mut count = 0;
    loop {
        let (_, page) = session.memory_ids_page(after, 128)?;
        if page.is_empty() {
            break;
        }
        count += page.len();
        after = page.last().copied();
        if page.len() < 128 {
            break;
        }
    }
    Ok(count)
}
fn count_phy(session: &PhySession) -> Result<usize, ProductError> {
    let mut after = None;
    let mut count = 0;
    loop {
        let (_, page) = session.memory_ids_page(after, 128)?;
        if page.is_empty() {
            break;
        }
        count += page.len();
        after = page.last().copied();
        if page.len() < 128 {
            break;
        }
    }
    Ok(count)
}

pub(crate) fn publication_collision(path: &Path) -> ProductError {
    ProductError::new(
        ProductErrorKind::PublicationCollision,
        format!("destination already exists: {}", path.display()),
    )
}

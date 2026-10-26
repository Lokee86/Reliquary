use crate::{
    ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRepositoryRef,
    ProjectRevisionCorrelation, ProjectRevisionRef, RelSemanticCut,
};

const MAGIC_V2: &[u8; 8] = b"PRJCOR02";
const MAX_FIELD_BYTES: usize = 32 * 1024;

pub fn encode(record: &ProjectRevisionCorrelation) -> Result<Vec<u8>, String> {
    let repository_id = field_bytes(
        "repository id",
        &record.project_revision.repository.repository_id,
    )?;
    let project_path = field_bytes(
        "project path",
        &record.project_revision.repository.project_path,
    )?;
    let revision = field_bytes("revision", &record.project_revision.revision)?;
    let kind = match record.project_revision.repository.kind {
        ProjectRepositoryKind::Lore => 1,
        ProjectRepositoryKind::Git => 2,
    };
    let management = match record.repository_management {
        ProjectRepositoryManagement::WarlockManaged => 1,
        ProjectRepositoryManagement::External => 2,
    };
    let mut out =
        Vec::with_capacity(54 + repository_id.len() + project_path.len() + revision.len());
    out.extend_from_slice(MAGIC_V2);
    out.extend_from_slice(&record.sequence.to_le_bytes());
    out.extend_from_slice(&record.rel_cut.global_version.to_le_bytes());
    out.extend_from_slice(&record.rel_cut.archive_version.to_le_bytes());
    out.extend_from_slice(&record.rel_cut.memory_version.to_le_bytes());
    out.push(kind);
    out.push(management);
    put_field(&mut out, repository_id)?;
    put_field(&mut out, project_path)?;
    put_field(&mut out, revision)?;
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Option<ProjectRevisionCorrelation>, String> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    if &bytes[..8] != MAGIC_V2 {
        return Ok(None);
    }
    let minimum = 42;
    if bytes.len() < minimum {
        return Err("truncated project correlation record".into());
    }
    let sequence = read_u64(bytes, 8)?;
    let rel_cut = RelSemanticCut {
        global_version: read_u64(bytes, 16)?,
        archive_version: read_u64(bytes, 24)?,
        memory_version: read_u64(bytes, 32)?,
    };
    let kind = match bytes[40] {
        1 => ProjectRepositoryKind::Lore,
        2 => ProjectRepositoryKind::Git,
        _ => return Err("invalid project repository kind".into()),
    };
    let repository_management = match bytes[41] {
        1 => ProjectRepositoryManagement::WarlockManaged,
        2 => ProjectRepositoryManagement::External,
        _ => return Err("invalid project repository management".into()),
    };
    let mut cursor = 42;
    let repository_id = read_field(bytes, &mut cursor, "repository id")?;
    let project_path = read_field(bytes, &mut cursor, "project path")?;
    let revision = read_field(bytes, &mut cursor, "revision")?;
    if cursor != bytes.len() {
        return Err("trailing bytes in project correlation record".into());
    }
    Ok(Some(ProjectRevisionCorrelation {
        sequence,
        rel_cut,
        project_revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind,
                repository_id,
                project_path,
            },
            revision,
        },
        repository_management,
    }))
}

fn field_bytes<'a>(name: &str, value: &'a str) -> Result<&'a [u8], String> {
    if value.len() > MAX_FIELD_BYTES {
        return Err(format!("project {name} exceeds {MAX_FIELD_BYTES} bytes"));
    }
    Ok(value.as_bytes())
}

fn put_field(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let len = u32::try_from(value.len()).map_err(|_| "project correlation field too large")?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn read_field(bytes: &[u8], cursor: &mut usize, name: &str) -> Result<String, String> {
    let len = read_u32(bytes, *cursor)? as usize;
    *cursor += 4;
    if len > MAX_FIELD_BYTES || bytes.len().saturating_sub(*cursor) < len {
        return Err(format!("invalid project {name} length"));
    }
    let value = std::str::from_utf8(&bytes[*cursor..*cursor + len])
        .map_err(|_| format!("project {name} is not UTF-8"))?
        .to_owned();
    *cursor += len;
    Ok(value)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated project correlation record".to_string())?;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let raw = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| "truncated project correlation record".to_string())?;
    Ok(u64::from_le_bytes(raw.try_into().unwrap()))
}

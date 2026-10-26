use crate::{
    FileId, ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
};

pub(crate) const PROJECT_FILE_BINDING_MAGIC: [u8; 8] = *b"PRJFILE1";
const MAX_FIELD_BYTES: usize = 32 * 1024;

pub(crate) fn encode(file_id: FileId, reference: &ProjectFileRef) -> Result<Vec<u8>, String> {
    let repository_id = field(
        "repository id",
        &reference.revision.repository.repository_id,
    )?;
    let project_path = field("project path", &reference.revision.repository.project_path)?;
    let revision = field("revision", &reference.revision.revision)?;
    let path = field("file path", &reference.path)?;
    let kind = match reference.revision.repository.kind {
        ProjectRepositoryKind::Lore => 1,
        ProjectRepositoryKind::Git => 2,
    };
    let mut out = Vec::with_capacity(
        78 + repository_id.len() + project_path.len() + revision.len() + path.len(),
    );
    out.extend_from_slice(&PROJECT_FILE_BINDING_MAGIC);
    out.extend_from_slice(&file_id.0);
    out.push(kind);
    out.push(u8::from(reference.content_hash.is_some()));
    if let Some(hash) = reference.content_hash {
        out.extend_from_slice(&hash);
    }
    put_field(&mut out, repository_id)?;
    put_field(&mut out, project_path)?;
    put_field(&mut out, revision)?;
    put_field(&mut out, path)?;
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Option<(FileId, ProjectFileRef)>, String> {
    if bytes.len() < 8 || bytes[..8] != PROJECT_FILE_BINDING_MAGIC {
        return Ok(None);
    }
    if bytes.len() < 42 {
        return Err("truncated project file binding".into());
    }
    let file_id = FileId(bytes[8..40].try_into().unwrap());
    let kind = match bytes[40] {
        1 => ProjectRepositoryKind::Lore,
        2 => ProjectRepositoryKind::Git,
        _ => return Err("invalid project file repository kind".into()),
    };
    let has_hash = match bytes[41] {
        0 => false,
        1 => true,
        _ => return Err("invalid project file hash flag".into()),
    };
    let mut cursor = 42;
    let content_hash = if has_hash {
        let end = cursor + 32;
        let raw = bytes
            .get(cursor..end)
            .ok_or_else(|| "truncated project file content hash".to_string())?;
        cursor = end;
        Some(raw.try_into().unwrap())
    } else {
        None
    };
    let repository_id = read_field(bytes, &mut cursor, "repository id")?;
    let project_path = read_field(bytes, &mut cursor, "project path")?;
    let revision = read_field(bytes, &mut cursor, "revision")?;
    let path = read_field(bytes, &mut cursor, "file path")?;
    if cursor != bytes.len() {
        return Err("trailing bytes in project file binding".into());
    }
    Ok(Some((
        file_id,
        ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind,
                    repository_id,
                    project_path,
                },
                revision,
            },
            path,
            content_hash,
        },
    )))
}

fn field<'a>(name: &str, value: &'a str) -> Result<&'a [u8], String> {
    if value.len() > MAX_FIELD_BYTES {
        return Err(format!(
            "project file {name} exceeds {MAX_FIELD_BYTES} bytes"
        ));
    }
    Ok(value.as_bytes())
}

fn put_field(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    let len = u32::try_from(value.len()).map_err(|_| "project file field too large")?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn read_field(bytes: &[u8], cursor: &mut usize, name: &str) -> Result<String, String> {
    let raw = bytes
        .get(*cursor..*cursor + 4)
        .ok_or_else(|| "truncated project file binding".to_string())?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor += 4;
    if len > MAX_FIELD_BYTES || bytes.len().saturating_sub(*cursor) < len {
        return Err(format!("invalid project file {name} length"));
    }
    let value = std::str::from_utf8(&bytes[*cursor..*cursor + len])
        .map_err(|_| format!("project file {name} is not UTF-8"))?
        .to_owned();
    *cursor += len;
    Ok(value)
}

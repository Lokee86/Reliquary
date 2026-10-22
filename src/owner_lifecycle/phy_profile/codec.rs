use super::model::{MAX_PHYLACTERY_PROFILE_NAME_BYTES, PhylacteryProfile};

pub(crate) const PHYLACTERY_PROFILE_MAGIC: [u8; 8] = *b"CVAPHYP1";

pub(crate) fn encode_phylactery_profile(profile: &PhylacteryProfile) -> Result<Vec<u8>, String> {
    validate_phylactery_profile(profile)?;
    let mut out = Vec::new();
    out.extend_from_slice(&PHYLACTERY_PROFILE_MAGIC);
    write_optional_string(&mut out, profile.display_name.as_deref())?;
    Ok(out)
}

pub(crate) fn decode_phylactery_profile(bytes: &[u8]) -> Result<Option<PhylacteryProfile>, String> {
    if bytes.len() < 8 || bytes[..8] != PHYLACTERY_PROFILE_MAGIC {
        return Ok(None);
    }
    let mut cursor = 8;
    let profile = PhylacteryProfile {
        display_name: read_optional_string(bytes, &mut cursor)?,
    };

    // Early feature builds wrote a second optional username field under the
    // same magic. Accept and discard it so those PHY files remain readable.
    if cursor < bytes.len() {
        let _legacy_username = read_optional_string(bytes, &mut cursor)?;
    }
    if cursor != bytes.len() {
        return Err("Phylactery profile has trailing bytes".into());
    }
    validate_phylactery_profile(&profile)?;
    Ok(Some(profile))
}

pub(crate) fn validate_phylactery_profile(profile: &PhylacteryProfile) -> Result<(), String> {
    validate_name("display name", profile.display_name.as_deref())
}

fn validate_name(label: &str, value: Option<&str>) -> Result<(), String> {
    if let Some(value) = value
        && (value.trim().is_empty() || value.len() > MAX_PHYLACTERY_PROFILE_NAME_BYTES)
    {
        return Err(format!("Phylactery {label} is empty or exceeds limit"));
    }
    Ok(())
}

fn write_optional_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value)
        }
        None => {
            out.push(0);
            Ok(())
        }
    }
}

fn read_optional_string(bytes: &[u8], cursor: &mut usize) -> Result<Option<String>, String> {
    let flag = *bytes
        .get(*cursor)
        .ok_or_else(|| "Phylactery profile is truncated".to_string())?;
    *cursor += 1;
    match flag {
        0 => Ok(None),
        1 => read_string(bytes, cursor).map(Some),
        _ => Err("Phylactery profile optional-string flag is invalid".into()),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    let len = u32::try_from(value.len()).map_err(|_| "Phylactery profile string is too large")?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, String> {
    let end_len = cursor
        .checked_add(4)
        .ok_or_else(|| "Phylactery profile length overflow".to_string())?;
    let raw_len = bytes
        .get(*cursor..end_len)
        .ok_or_else(|| "Phylactery profile is truncated".to_string())?;
    *cursor = end_len;
    let len = u32::from_le_bytes(raw_len.try_into().unwrap()) as usize;
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| "Phylactery profile string length overflow".to_string())?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or_else(|| "Phylactery profile string is truncated".to_string())?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| "Phylactery profile string is not UTF-8".into())
}

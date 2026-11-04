use super::*;

pub(super) fn put_str(b: &mut Vec<u8>, s: &str) -> Result<(), InsomniaError> {
    let n = u32::try_from(s.len()).map_err(|_| InsomniaError::InvalidField("string length"))?;
    b.extend_from_slice(&n.to_le_bytes());
    b.extend_from_slice(s.as_bytes());
    Ok(())
}
pub(super) fn put_opt_str(b: &mut Vec<u8>, s: Option<&str>) -> Result<(), InsomniaError> {
    match s {
        None => b.push(0),
        Some(s) => {
            b.push(1);
            put_str(b, s)?
        }
    }
    Ok(())
}
pub(super) fn put_opt_bytes(b: &mut Vec<u8>, v: Option<&[u8; 32]>) {
    match v {
        None => b.push(0),
        Some(v) => {
            b.push(1);
            b.extend_from_slice(v)
        }
    }
}
pub(super) fn put_opt_payload(
    b: &mut Vec<u8>,
    payload: Option<&[u8]>,
) -> Result<(), InsomniaError> {
    match payload {
        None => b.push(0),
        Some(payload) => {
            if payload.len() > MAX_HELD_PAYLOAD_BYTES {
                return Err(InsomniaError::InvalidField("held payload size"));
            }
            b.push(1);
            let len = u32::try_from(payload.len())
                .map_err(|_| InsomniaError::InvalidField("held payload size"))?;
            b.extend_from_slice(&len.to_le_bytes());
            b.extend_from_slice(payload);
        }
    }
    Ok(())
}
pub(super) fn put_opt_i64(b: &mut Vec<u8>, v: Option<i64>) {
    match v {
        None => b.push(0),
        Some(v) => {
            b.push(1);
            b.extend_from_slice(&v.to_le_bytes())
        }
    }
}
pub(super) fn put_ids(b: &mut Vec<u8>, v: &[Id]) -> Result<(), InsomniaError> {
    b.extend_from_slice(
        &u32::try_from(v.len())
            .map_err(|_| InsomniaError::InvalidField("memory IDs"))?
            .to_le_bytes(),
    );
    for x in v {
        b.extend_from_slice(x)
    }
    Ok(())
}
type Id = [u8; 32];
pub(super) fn get_ids(b: &[u8], p: &mut usize) -> Result<Vec<Id>, InsomniaError> {
    let n = u32::from_le_bytes(take::<4>(b, p)?) as usize;
    if n > MAX_RECEIPT_IDS || n > b.len().saturating_sub(*p) / 32 {
        return Err(InsomniaError::CorruptRecord("memory ID count"));
    }
    (0..n).map(|_| take::<32>(b, p)).collect()
}
pub(super) fn get_str(b: &[u8], p: &mut usize) -> Result<String, InsomniaError> {
    let n = u32::from_le_bytes(take::<4>(b, p)?) as usize;
    if n > MAX_RECEIPT_TEXT_BYTES {
        return Err(InsomniaError::CorruptRecord("text too large"));
    }
    String::from_utf8(take_vec(b, p, n)?).map_err(|_| InsomniaError::CorruptRecord("invalid UTF-8"))
}
pub(super) fn get_opt_str(b: &[u8], p: &mut usize) -> Result<Option<String>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => Ok(Some(get_str(b, p)?)),
        _ => Err(InsomniaError::CorruptRecord("optional string tag")),
    }
}
pub(super) fn get_opt_i64(b: &[u8], p: &mut usize) -> Result<Option<i64>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => Ok(Some(i64::from_le_bytes(take::<8>(b, p)?))),
        _ => Err(InsomniaError::CorruptRecord("optional integer tag")),
    }
}
pub(super) fn get_opt_array32(b: &[u8], p: &mut usize) -> Result<Option<[u8; 32]>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => Ok(Some(take::<32>(b, p)?)),
        _ => Err(InsomniaError::CorruptRecord("optional token tag")),
    }
}
pub(super) fn get_opt_payload(b: &[u8], p: &mut usize) -> Result<Option<Vec<u8>>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => {
            let len = u32::from_le_bytes(take::<4>(b, p)?) as usize;
            if len > MAX_HELD_PAYLOAD_BYTES {
                return Err(InsomniaError::CorruptRecord("held payload too large"));
            }
            Ok(Some(take_vec(b, p, len)?))
        }
        _ => Err(InsomniaError::CorruptRecord("optional held payload tag")),
    }
}
pub(super) fn byte(b: &[u8], p: &mut usize) -> Result<u8, InsomniaError> {
    Ok(take::<1>(b, p)?[0])
}
pub(super) fn take<const N: usize>(b: &[u8], p: &mut usize) -> Result<[u8; N], InsomniaError> {
    let end = p
        .checked_add(N)
        .ok_or(InsomniaError::CorruptRecord("length overflow"))?;
    let s = b
        .get(*p..end)
        .ok_or(InsomniaError::CorruptRecord("truncated record"))?;
    *p = end;
    Ok(s.try_into().unwrap())
}
pub(super) fn take_vec(b: &[u8], p: &mut usize, n: usize) -> Result<Vec<u8>, InsomniaError> {
    let end = p
        .checked_add(n)
        .ok_or(InsomniaError::CorruptRecord("length overflow"))?;
    let v = b
        .get(*p..end)
        .ok_or(InsomniaError::CorruptRecord("truncated bytes"))?
        .to_vec();
    *p = end;
    Ok(v)
}
pub(super) fn end(b: &[u8], p: usize) -> Result<(), InsomniaError> {
    if p == b.len() {
        Ok(())
    } else {
        Err(InsomniaError::CorruptRecord("trailing bytes"))
    }
}

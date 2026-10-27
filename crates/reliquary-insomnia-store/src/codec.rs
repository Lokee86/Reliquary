use crate::{
    EpisodeId, InsomniaAttempt, InsomniaError, InsomniaLeaseToken, InsomniaPriority, InsomniaWork,
    InsomniaWorkState, MemoryId,
};
const FORMAT: &[u8; 8] = b"CVAINSF1";
const WORK: &[u8; 8] = b"CVAINSW1";
const ATTEMPT: &[u8; 8] = b"CVAINSA1";
const FORMAT_FAMILY: &[u8; 7] = b"CVAINSF";
const WORK_FAMILY: &[u8; 7] = b"CVAINSW";
const ATTEMPT_FAMILY: &[u8; 7] = b"CVAINSA";
pub enum InsomniaRecord {
    Work(InsomniaWork),
    Attempt(InsomniaAttempt),
}
pub fn encode_format() -> Vec<u8> {
    FORMAT.to_vec()
}
pub fn decode_format(b: &[u8]) -> Result<bool, InsomniaError> {
    if !b.starts_with(FORMAT_FAMILY) {
        return Ok(false);
    }
    if b.len() < 8 {
        return Err(InsomniaError::CorruptRecord("short insomnia format tag"));
    }
    if &b[..8] != FORMAT {
        return Err(InsomniaError::CorruptRecord(
            "unsupported insomnia format version",
        ));
    }
    if b.len() != 8 {
        return Err(InsomniaError::CorruptRecord(
            "insomnia format trailing bytes",
        ));
    }
    Ok(true)
}
pub fn encode_work(w: &InsomniaWork) -> Result<Vec<u8>, InsomniaError> {
    let mut o = Vec::new();
    o.extend_from_slice(WORK);
    o.extend_from_slice(&w.episode_id.0);
    o.push(w.priority.tag());
    o.push(w.state.tag());
    o.extend_from_slice(&w.attempt_count.to_le_bytes());
    o.extend_from_slice(&w.updated_at_ns.to_le_bytes());
    ws(&mut o, w.lease_owner.as_deref())?;
    wt(&mut o, w.lease_token);
    wi(&mut o, w.lease_expires_ns);
    wi(&mut o, w.retry_after_ns);
    ws(&mut o, w.last_error.as_deref())?;
    Ok(o)
}
pub fn encode_attempt(a: &InsomniaAttempt) -> Result<Vec<u8>, InsomniaError> {
    let mut o = Vec::new();
    o.extend_from_slice(ATTEMPT);
    o.extend_from_slice(&a.episode_id.0);
    o.extend_from_slice(&a.attempt.to_le_bytes());
    o.push(a.state.tag());
    o.extend_from_slice(&a.started_at_ns.to_le_bytes());
    o.extend_from_slice(&a.completed_at_ns.to_le_bytes());
    o.extend_from_slice(&a.rejected_count.to_le_bytes());
    string(&mut o, &a.extractor_model)?;
    string(&mut o, &a.extractor_version)?;
    let n =
        u32::try_from(a.memory_ids.len()).map_err(|_| InsomniaError::InvalidField("memory ids"))?;
    o.extend_from_slice(&n.to_le_bytes());
    for id in &a.memory_ids {
        o.extend_from_slice(&id.0)
    }
    ws(&mut o, a.error.as_deref())?;
    Ok(o)
}
pub fn decode_record(b: &[u8]) -> Result<Option<InsomniaRecord>, InsomniaError> {
    if b.starts_with(WORK_FAMILY) {
        if b.len() < 8 {
            return Err(InsomniaError::CorruptRecord("short insomnia work tag"));
        }
        if &b[..8] != WORK {
            return Err(InsomniaError::CorruptRecord(
                "unsupported insomnia work version",
            ));
        }
        return decode_work(b).map(|x| Some(InsomniaRecord::Work(x)));
    }
    if b.starts_with(ATTEMPT_FAMILY) {
        if b.len() < 8 {
            return Err(InsomniaError::CorruptRecord("short insomnia attempt tag"));
        }
        if &b[..8] != ATTEMPT {
            return Err(InsomniaError::CorruptRecord(
                "unsupported insomnia attempt version",
            ));
        }
        return decode_attempt(b).map(|x| Some(InsomniaRecord::Attempt(x)));
    }
    Ok(None)
}
fn decode_work(b: &[u8]) -> Result<InsomniaWork, InsomniaError> {
    if b.len() < 54 {
        return Err(InsomniaError::CorruptRecord("short insomnia work"));
    }
    let episode_id = EpisodeId(b[8..40].try_into().unwrap());
    let priority = InsomniaPriority::from_tag(b[40])
        .ok_or(InsomniaError::CorruptRecord("invalid insomnia priority"))?;
    let state = InsomniaWorkState::from_tag(b[41])
        .ok_or(InsomniaError::CorruptRecord("invalid insomnia state"))?;
    let attempt_count = u32::from_le_bytes(b[42..46].try_into().unwrap());
    let updated_at_ns = i64::from_le_bytes(b[46..54].try_into().unwrap());
    let mut c = 54;
    let lease_owner = rs(b, &mut c)?;
    let lease_token = rt(b, &mut c)?;
    let lease_expires_ns = ri(b, &mut c)?;
    let retry_after_ns = ri(b, &mut c)?;
    let last_error = rs(b, &mut c)?;
    end(b, c, "insomnia work trailing bytes")?;
    Ok(InsomniaWork {
        episode_id,
        priority,
        state,
        attempt_count,
        lease_owner,
        lease_token,
        lease_expires_ns,
        retry_after_ns,
        last_error,
        updated_at_ns,
    })
}
fn decode_attempt(b: &[u8]) -> Result<InsomniaAttempt, InsomniaError> {
    if b.len() < 65 {
        return Err(InsomniaError::CorruptRecord("short insomnia attempt"));
    }
    let episode_id = EpisodeId(b[8..40].try_into().unwrap());
    let attempt = u32::from_le_bytes(b[40..44].try_into().unwrap());
    let state = InsomniaWorkState::from_tag(b[44])
        .ok_or(InsomniaError::CorruptRecord("invalid attempt state"))?;
    let started_at_ns = i64::from_le_bytes(b[45..53].try_into().unwrap());
    let completed_at_ns = i64::from_le_bytes(b[53..61].try_into().unwrap());
    let rejected_count = u32::from_le_bytes(b[61..65].try_into().unwrap());
    let mut c = 65;
    let extractor_model = rs0(b, &mut c)?;
    let extractor_version = rs0(b, &mut c)?;
    let count = take(b, &mut c, 4, "missing memory id count")?;
    let n = u32::from_le_bytes(count.try_into().unwrap()) as usize;
    // Check the declared count before reserving so a short corrupt payload cannot
    // trigger an attacker-controlled large allocation.
    if n > b.len().saturating_sub(c) / 32 {
        return Err(InsomniaError::CorruptRecord("truncated memory id"));
    }
    let mut memory_ids = Vec::with_capacity(n);
    for _ in 0..n {
        memory_ids.push(MemoryId(
            take(b, &mut c, 32, "truncated memory id")?
                .try_into()
                .unwrap(),
        ))
    }
    let error = rs(b, &mut c)?;
    end(b, c, "insomnia attempt trailing bytes")?;
    Ok(InsomniaAttempt {
        episode_id,
        attempt,
        state,
        started_at_ns,
        completed_at_ns,
        extractor_model,
        extractor_version,
        memory_ids,
        external_memory_refs: Vec::new(),
        rejected_count,
        error,
    })
}
fn string(o: &mut Vec<u8>, s: &str) -> Result<(), InsomniaError> {
    let n = u32::try_from(s.len()).map_err(|_| InsomniaError::InvalidField("string"))?;
    o.extend_from_slice(&n.to_le_bytes());
    o.extend_from_slice(s.as_bytes());
    Ok(())
}
fn ws(o: &mut Vec<u8>, s: Option<&str>) -> Result<(), InsomniaError> {
    match s {
        Some(s) => {
            o.push(1);
            string(o, s)
        }
        None => {
            o.push(0);
            Ok(())
        }
    }
}
fn wt(o: &mut Vec<u8>, x: Option<InsomniaLeaseToken>) {
    match x {
        Some(t) => {
            o.push(1);
            o.extend_from_slice(&t.0)
        }
        None => o.push(0),
    }
}
fn wi(o: &mut Vec<u8>, x: Option<i64>) {
    match x {
        Some(x) => {
            o.push(1);
            o.extend_from_slice(&x.to_le_bytes())
        }
        None => o.push(0),
    }
}
fn take<'a>(
    b: &'a [u8],
    c: &mut usize,
    n: usize,
    msg: &'static str,
) -> Result<&'a [u8], InsomniaError> {
    let e = c
        .checked_add(n)
        .ok_or(InsomniaError::InvalidField("record"))?;
    let v = b.get(*c..e).ok_or(InsomniaError::CorruptRecord(msg))?;
    *c = e;
    Ok(v)
}
fn rs0(b: &[u8], c: &mut usize) -> Result<String, InsomniaError> {
    let n =
        u32::from_le_bytes(take(b, c, 4, "missing string length")?.try_into().unwrap()) as usize;
    String::from_utf8(take(b, c, n, "truncated string")?.to_vec())
        .map_err(|_| InsomniaError::CorruptRecord("invalid utf8"))
}
fn rs(b: &[u8], c: &mut usize) -> Result<Option<String>, InsomniaError> {
    match take(b, c, 1, "missing optional string flag")?[0] {
        0 => Ok(None),
        1 => rs0(b, c).map(Some),
        _ => Err(InsomniaError::CorruptRecord("invalid optional string flag")),
    }
}
fn rt(b: &[u8], c: &mut usize) -> Result<Option<InsomniaLeaseToken>, InsomniaError> {
    match take(b, c, 1, "missing lease token flag")?[0] {
        0 => Ok(None),
        1 => Ok(Some(InsomniaLeaseToken(
            take(b, c, 32, "truncated lease token")?.try_into().unwrap(),
        ))),
        _ => Err(InsomniaError::CorruptRecord("invalid lease token flag")),
    }
}
fn ri(b: &[u8], c: &mut usize) -> Result<Option<i64>, InsomniaError> {
    match take(b, c, 1, "missing optional i64 flag")?[0] {
        0 => Ok(None),
        1 => Ok(Some(i64::from_le_bytes(
            take(b, c, 8, "truncated optional i64")?.try_into().unwrap(),
        ))),
        _ => Err(InsomniaError::CorruptRecord("invalid optional i64 flag")),
    }
}
fn end(b: &[u8], c: usize, msg: &'static str) -> Result<(), InsomniaError> {
    if c != b.len() {
        Err(InsomniaError::CorruptRecord(msg))
    } else {
        Ok(())
    }
}

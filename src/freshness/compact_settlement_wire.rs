//! Canonical bounded wire primitives for the single current settlement grammar.
use super::{Reader, put_record};
use crate::freshness::FreshnessRecord;

pub(super) fn put_var(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
pub(super) fn put_count(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "compact settlement count overflow")?;
    put_var(out, u64::from(value));
    Ok(())
}
pub(super) fn put_signed(out: &mut Vec<u8>, value: i16) {
    let value = i64::from(value);
    put_var(out, ((value << 1) ^ (value >> 63)) as u64);
}
pub(super) fn encoded_record(record: FreshnessRecord) -> Vec<u8> {
    let mut out = Vec::new();
    put_record(&mut out, record);
    out
}
pub(super) struct CompactReader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> CompactReader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(count)
            .ok_or("compact extent overflow")?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated compact settlement")?;
        self.at = end;
        Ok(value)
    }
    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?
            .try_into()
            .map_err(|_| "invalid compact extent".into())
    }
    pub(super) fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub(super) fn var(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        for index in 0..10 {
            let byte = self.take(1)?[0];
            if index == 9 && byte > 1 {
                return Err("compact varint overflow".into());
            }
            value |= u64::from(byte & 127) << (index * 7);
            if byte & 128 == 0 {
                if index > 0 && byte == 0 {
                    return Err("nonminimal compact varint".into());
                }
                return Ok(value);
            }
        }
        Err("compact varint overflow".into())
    }
    pub(super) fn count(&mut self, minimum_bytes: usize) -> Result<usize, String> {
        let count = u32::try_from(self.var()?).map_err(|_| "compact count overflow")? as usize;
        if minimum_bytes == 0 || count > self.remaining() / minimum_bytes {
            return Err("compact count exceeds remaining bytes".into());
        }
        Ok(count)
    }
    pub(super) fn index(&mut self, count: usize) -> Result<usize, String> {
        let index = usize::try_from(self.var()?).map_err(|_| "compact index overflow")?;
        if index >= count {
            return Err("compact index outside dictionary".into());
        }
        Ok(index)
    }
    pub(super) fn signed(&mut self) -> Result<i16, String> {
        let value = self.var()?;
        let signed = ((value >> 1) as i64) ^ -((value & 1) as i64);
        i16::try_from(signed).map_err(|_| "compact signed scalar overflow".into())
    }
    pub(super) fn record(&mut self) -> Result<FreshnessRecord, String> {
        let count = self.count(1)?;
        let bytes = self.take(count)?;
        let mut reader = Reader { bytes, at: 0 };
        let record = reader.record()?;
        if reader.at != bytes.len() {
            return Err("trailing compact anchor bytes".into());
        }
        Ok(record)
    }
    pub(super) fn finish(self) -> Result<(), String> {
        if self.at != self.bytes.len() {
            return Err("trailing compact settlement bytes".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimal_varints_round_trip_boundaries_and_reject_malleability() {
        for value in [0, 1, 127, 128, 16383, 16384, u32::MAX as u64, u64::MAX] {
            let mut bytes = Vec::new();
            put_var(&mut bytes, value);
            let mut r = CompactReader::new(&bytes);
            assert_eq!(r.var().unwrap(), value);
            r.finish().unwrap();
        }
        for bytes in [
            vec![128],
            vec![128, 0],
            vec![129, 0],
            vec![255; 10],
            vec![128, 128, 128, 128, 128, 128, 128, 128, 128, 2],
        ] {
            assert!(CompactReader::new(&bytes).var().is_err());
        }
    }
    #[test]
    fn signed_scalars_and_bounded_counts_refuse_overflow_and_trailing_bytes() {
        for value in [i16::MIN, -200, -1, 0, 1, 200, i16::MAX] {
            let mut bytes = Vec::new();
            put_signed(&mut bytes, value);
            assert_eq!(CompactReader::new(&bytes).signed().unwrap(), value);
        }
        assert!(CompactReader::new(&[5, 0]).count(1).is_err());
        assert!(CompactReader::new(&[0]).index(0).is_err());
        assert!(CompactReader::new(&[0]).finish().is_err());
        let mut bytes = Vec::new();
        put_var(&mut bytes, 65536);
        assert!(CompactReader::new(&bytes).signed().is_err());
    }
}

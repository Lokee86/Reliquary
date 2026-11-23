use super::*;

const HEADER_BYTES: usize = 32;
const OFFSET_BYTES: usize = 16;
const INVALID: ArchiveError = ArchiveError::CorruptRecord("Archive lexical index encoding");

impl FragmentLexicalIndex {
    pub fn encode_into(&self, output: &mut Vec<u8>) {
        for value in [
            self.rows,
            self.offsets.len(),
            self.dictionary.len(),
            self.postings.len(),
        ] {
            output.extend_from_slice(&(value as u64).to_le_bytes());
        }
        for offset in &self.offsets {
            output.extend_from_slice(&(offset.word as u64).to_le_bytes());
            output.extend_from_slice(&(offset.posting as u64).to_le_bytes());
        }
        output.extend_from_slice(self.dictionary.as_bytes());
        output.extend_from_slice(&self.postings);
    }

    pub fn decode(bytes: &[u8], expected_rows: usize) -> Result<Self, ArchiveError> {
        let (header, body) = bytes.split_at_checked(HEADER_BYTES).ok_or(INVALID)?;
        let [rows, offsets, dictionary_bytes, posting_bytes] =
            header.as_chunks::<8>().0.try_into().map_err(|_| INVALID)?;
        let rows = read_size(rows)?;
        let offsets = read_size(offsets)?;
        if rows != expected_rows || offsets == 0 {
            return Err(INVALID);
        }
        let offset_bytes = offsets.checked_mul(OFFSET_BYTES).ok_or(INVALID)?;
        let (offsets, rest) = body.split_at_checked(offset_bytes).ok_or(INVALID)?;
        let (dictionary, postings) = rest
            .split_at_checked(read_size(dictionary_bytes)?)
            .ok_or(INVALID)?;
        if postings.len() != read_size(posting_bytes)? {
            return Err(INVALID);
        }
        let dictionary = std::str::from_utf8(dictionary).map_err(|_| INVALID)?;
        let offsets = offsets
            .as_chunks::<OFFSET_BYTES>()
            .0
            .iter()
            .map(|entry| {
                let (word, posting) = entry.split_at(8);
                Ok(TermOffset {
                    word: read_size(word.try_into().map_err(|_| INVALID)?)?,
                    posting: read_size(posting.try_into().map_err(|_| INVALID)?)?,
                })
            })
            .collect::<Result<Vec<_>, ArchiveError>>()?;
        validate_offsets(&offsets, dictionary, postings, rows)?;
        Ok(Self {
            rows,
            dictionary: dictionary.into(),
            offsets: offsets.into_boxed_slice(),
            postings: postings.into(),
        })
    }
}

fn read_size(bytes: [u8; 8]) -> Result<usize, ArchiveError> {
    usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| INVALID)
}

fn validate_offsets(
    offsets: &[TermOffset],
    dictionary: &str,
    postings: &[u8],
    rows: usize,
) -> Result<(), ArchiveError> {
    let first = offsets.first().ok_or(INVALID)?;
    let end = offsets.last().ok_or(INVALID)?;
    if first.word != 0
        || first.posting != 0
        || end.word != dictionary.len()
        || end.posting != postings.len()
    {
        return Err(INVALID);
    }
    let mut previous_word = "";
    for pair in offsets.windows(2) {
        let [start, end] = pair else {
            return Err(INVALID);
        };
        let word = dictionary.get(start.word..end.word).ok_or(INVALID)?;
        let list = postings.get(start.posting..end.posting).ok_or(INVALID)?;
        if word <= previous_word || list.is_empty() {
            return Err(INVALID);
        }
        validate_postings(list, rows)?;
        previous_word = word;
    }
    Ok(())
}

fn validate_postings(mut list: &[u8], rows: usize) -> Result<(), ArchiveError> {
    let mut row = 0usize;
    let mut first = true;
    while !list.is_empty() {
        let value = usize::try_from(read_varint(&mut list).ok_or(INVALID)?).map_err(|_| INVALID)?;
        let delta = value >> FREQUENCY_BITS;
        row = row.checked_add(delta).ok_or(INVALID)?;
        if row >= rows || (!first && delta == 0) {
            return Err(INVALID);
        }
        first = false;
    }
    Ok(())
}

fn read_varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0;
    for shift in (0..64).step_by(7) {
        let (&byte, next) = bytes.split_first()?;
        *bytes = next;
        if shift == 63 && byte > 1 {
            return None;
        }
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return (shift == 0 || byte > 0).then_some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_codec_roundtrips_and_refuses_invalid_postings() {
        let mut texts = vec![None; 600];
        texts[0] = Some(vec![Arc::new("Café ana banana".into())]);
        texts[599] = Some(vec![Arc::new("CAFÉ".into())]);
        let index = FragmentLexicalIndex::build(&texts).unwrap();
        let mut bytes = Vec::new();
        index.encode_into(&mut bytes);
        let decoded = FragmentLexicalIndex::decode(&bytes, 600).unwrap();
        let query = lexical_terms("café ana missing");
        assert_eq!(decoded.stats(), index.stats());
        assert_eq!(decoded.counts(&query), index.counts(&query));
        assert!(FragmentLexicalIndex::decode(&bytes, 599).is_err());
        assert!(FragmentLexicalIndex::decode(&bytes[..bytes.len() - 1], 600).is_err());
        *bytes.last_mut().unwrap() = 128;
        assert!(FragmentLexicalIndex::decode(&bytes, 600).is_err());
    }
}

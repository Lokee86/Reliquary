use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub(super) const PAGE_BYTES: usize = 256 * 1024;
pub(super) const MAX_PROJECTION_BYTES: usize = 256 * 1024 * 1024;

fn digest(seal: &[u8; 32], index: u64, bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(seal);
    hash.update(index.to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

/// One fixed buffer while serializing a typed projection.
pub(super) struct PageWriter<'a> {
    file: &'a mut dyn Write,
    buffer: Vec<u8>,
    seal: [u8; 32],
    total: usize,
    index: u64,
}

impl<'a> PageWriter<'a> {
    pub fn new(file: &'a mut dyn Write, seal: [u8; 32]) -> Self {
        Self {
            file,
            buffer: Vec::with_capacity(PAGE_BYTES),
            seal,
            total: 0,
            index: 0,
        }
    }

    pub fn finish(mut self) -> io::Result<usize> {
        if !self.buffer.is_empty() {
            self.commit_page()?;
        }
        Ok(self.total)
    }

    fn commit_page(&mut self) -> io::Result<()> {
        self.file
            .write_all(&digest(&self.seal, self.index, &self.buffer))?;
        self.file.write_all(&self.buffer)?;
        self.buffer.clear();
        self.index += 1;
        Ok(())
    }
}

impl Write for PageWriter<'_> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let count = data.len().min(PAGE_BYTES - self.buffer.len());
        if self.total + count > MAX_PROJECTION_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint state exceeds cache limit",
            ));
        }
        self.buffer.extend_from_slice(&data[..count]);
        self.total += count;
        if self.buffer.len() == PAGE_BYTES {
            self.commit_page()?;
        }
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub(super) struct PageReader<'a> {
    file: &'a mut dyn Read,
    buffer: Vec<u8>,
    seal: [u8; 32],
    consumed: usize,
    remaining: usize,
    index: u64,
}

impl<'a> PageReader<'a> {
    pub fn new(file: &'a mut dyn Read, seal: [u8; 32], remaining: usize) -> Self {
        Self {
            file,
            buffer: Vec::with_capacity(PAGE_BYTES),
            seal,
            consumed: 0,
            remaining,
            index: 0,
        }
    }

    pub fn is_exhausted(&self) -> bool {
        self.consumed == self.buffer.len() && self.remaining == 0
    }

    fn load_page(&mut self) -> io::Result<()> {
        let len = self.remaining.min(PAGE_BYTES);
        let mut expected = [0; 32];
        self.file.read_exact(&mut expected)?;
        self.buffer.resize(len, 0);
        self.file.read_exact(&mut self.buffer)?;
        if digest(&self.seal, self.index, &self.buffer) != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint page checksum",
            ));
        }
        self.consumed = 0;
        self.remaining -= len;
        self.index += 1;
        Ok(())
    }
}

impl Read for PageReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.is_exhausted() {
            return Ok(0);
        }
        if self.consumed == self.buffer.len() {
            self.load_page()?;
        }
        let len = output.len().min(self.buffer.len() - self.consumed);
        output[..len].copy_from_slice(&self.buffer[self.consumed..self.consumed + len]);
        self.consumed += len;
        Ok(len)
    }
}

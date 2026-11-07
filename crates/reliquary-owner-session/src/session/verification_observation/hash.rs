use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::io::Read;

pub(super) fn hash_open_container(container: &Container) -> Result<[u8; 32], String> {
    let mut reader = container
        .physical_bytes()
        .map_err(|_| "candidate_hash_open_failed".to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| "candidate_hash_read_failed".to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

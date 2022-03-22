use algonaut::crypto::HashDigest;
use anyhow::Result;
use serde::Serialize;
use sha2::Digest;

pub trait Hashable: Serialize {
    fn compute_hash(&self) -> Result<HashResult> {
        let bytes = self.bytes_to_hash()?;
        Ok(HashResult {
            hash: hash(&bytes),
            hashed_bytes: bytes,
        })
    }

    fn bytes_to_hash(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }
}

/// Not using this since using app state to store dao, but ok to keep
#[allow(dead_code)]
pub fn hash(bytes: &[u8]) -> HashDigest {
    HashDigest(sha2::Sha512_256::digest(bytes).into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
// TODO put in separate file, to prevent constr. with mismatching hash
pub struct HashResult {
    hash: HashDigest,
    pub hashed_bytes: Vec<u8>, // the payload that was hashed
}

impl HashResult {
    #[allow(dead_code)]
    pub fn hash(&self) -> &HashDigest {
        &self.hash
    }
}

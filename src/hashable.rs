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

pub fn hash(bytes: &[u8]) -> HashDigest {
    HashDigest(sha2::Sha512Trunc256::digest(&bytes).into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
// TODO put in separate file, to prevent constr. with mismatching hash
pub struct HashResult {
    hash: HashDigest,
    pub hashed_bytes: Vec<u8>, // the payload that was hashed
}

impl HashResult {
    pub fn hash(&self) -> &HashDigest {
        &self.hash
    }
}

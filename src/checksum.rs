use async_trait::async_trait;

#[cfg(feature = "sha256")]
use sha2::{Digest, Sha256};

#[cfg(feature = "md5")]
use md5::compute;

#[async_trait]
pub trait Checksummer: Sync + Send {
    /// Returns a checksum for the given data.
    fn checksum(&self, data: Vec<u8>) -> String;
}

#[cfg(feature = "sha256")]
pub struct Sha256Checksummer {}

#[cfg(feature = "sha256")]
impl Checksummer for Sha256Checksummer {
    fn checksum(&self, data: Vec<u8>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        format!("sha256-{:x}", result)
    }
}

#[cfg(feature = "md5")]
pub struct Md5Checksummer {}

#[cfg(feature = "md5")]
impl Checksummer for Md5Checksummer {
    fn checksum(&self, data: Vec<u8>) -> String {
        let digest = compute(data);
        format!("md5-{:x}", digest)
    }
}

pub fn get_checksummer(checksum_type: &str) -> Box<dyn Checksummer> {
    match checksum_type {
        #[cfg(feature = "sha256")]
        "sha256" => Box::new(Sha256Checksummer {}),

        #[cfg(feature = "md5")]
        "md5" => Box::new(Md5Checksummer {}),

        _ => panic!("Unknown checksum type: {}", checksum_type),
    }
}

use std::fmt::Display;

use algonaut::core::{to_app_address, Address};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapiAppId(pub u64);

impl CapiAppId {
    pub fn address(&self) -> Address {
        to_app_address(self.0)
    }
}
impl Display for CapiAppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

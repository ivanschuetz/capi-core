use std::cmp::Ordering;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// An amount of shares (DAO ASA)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareAmount(pub u64);

impl Display for ShareAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialOrd for ShareAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

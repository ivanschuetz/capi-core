use std::error::Error;

use algonaut::core::{Address, MicroAlgos};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type DefaultError = Box<dyn Error + Send + Sync>;

/// Note that we don't send things that can be queried from the blockchain,
/// like the asset name or supply
/// This is to minimize the off chain reponsibilities,
/// everything that can be queried directly from the blockchain should be (unless there's a very good reason not to)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectForUsers {
    pub id: String,
    pub name: String,
    pub asset_price: MicroAlgos,
    pub investors_share: u64,
    pub shares_asset_id: u64,
    pub central_app_id: u64,
    pub invest_escrow_address: Address,
    pub staking_escrow_address: Address,
    pub central_escrow_address: Address,
    pub customer_escrow_address: Address,
    pub invest_link: String,
    pub my_investment_link: String,
    pub project_link: String,
    pub creator: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Withdrawal {
    pub project_id: u64,
    pub amount: MicroAlgos,
    pub description: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedWithdrawal {
    pub id: String, // saved: db id
    pub project_id: String,
    pub amount: MicroAlgos,
    pub description: String,
    pub date: DateTime<Utc>,
}

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
    pub vote_threshold: u64, // percent
    pub shares_asset_id: u64,
    pub central_app_id: u64,
    pub slot_ids: Vec<u64>,
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
pub struct WithdrawalRequestInputs {
    pub project_id: String,
    pub slot_id: String,
    pub amount: MicroAlgos,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WithdrawalRequest {
    pub project_id: String,
    pub slot_id: String,
    pub amount: MicroAlgos,
    pub description: String,
    pub date: DateTime<Utc>,
    // temporary hack - we most likely have to use the indexer to check whether withdrawals are complete
    // (save tx id when submitting withdrawal, check if there's later withdrawal tx for same app?)
    // ideally off chain backend doesn't know if it's complete or not, just tx id -> metadata (descr etc.)
    // TODO consider storing metadata in the tx note field? so we don't need this table at all?
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedWithdrawalRequest {
    pub id: String, // saved: db id
    pub slot_id: String,
    pub project_id: String,
    pub amount: MicroAlgos,
    pub description: String,
    pub date: DateTime<Utc>,
    pub complete: bool,
}

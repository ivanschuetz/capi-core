use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    indexer::v2::Indexer,
};
use anyhow::{Result};
use chrono::{DateTime, Utc};
use crate::{
    flows::{create_project::{
        create_project::Escrows,
        storage::{
            load_project::{ ProjectId, TxId},
        },
    }, withdraw::withdrawals::withdrawals},
};

use super::received_payments::received_payments;

#[derive(Debug, Clone)]
pub struct FundsActivityEntry {
    pub date: DateTime<Utc>,
    pub type_: FundsActivityEntryType,
    pub description: String,
    pub amount: MicroAlgos,
    pub tx_id: TxId,
}

#[derive(Debug, Clone)]
pub enum FundsActivityEntryType {
    Income, Spending
}

pub async fn funds_activity(
    algod: &Algod,
    indexer: &Indexer,
    creator: &Address,
    project_id: &ProjectId,
    customer_escrow_address: &Address,
    escrows: &Escrows,
) -> Result<Vec<FundsActivityEntry>> {

    let withdrawals = withdrawals(algod, indexer, creator, project_id, escrows).await?;
    let payments = received_payments(indexer, customer_escrow_address).await?;

    let mut funds_activity = vec![];

    for withdrawal in withdrawals {
        funds_activity.push(FundsActivityEntry {
            date: withdrawal.date,
            type_: FundsActivityEntryType::Spending,
            description: withdrawal.description,
            amount: withdrawal.amount,
            tx_id: withdrawal.tx_id.clone(),
        })
    }

    for payment in payments {
        funds_activity.push(FundsActivityEntry {
            date: payment.date,
            type_: FundsActivityEntryType::Income,
            description: payment.note.unwrap_or("No description provided".to_owned()),
            amount: payment.amount,
            tx_id: payment.tx_id.clone(),
        })
    }

    // sort ascendingly by date
    funds_activity.sort_by(|p1, p2| p1.date.cmp(&p2.date));

    Ok(funds_activity)
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub amount: MicroAlgos,
    pub sender: Address,
    pub date: DateTime<Utc>,
    pub note: Option<String>,
}

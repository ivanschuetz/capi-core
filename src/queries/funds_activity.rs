use crate::{
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    flows::{
        create_dao::{
            create_dao::Escrows,
            storage::load_dao::{DaoId, TxId},
        },
        withdraw::withdrawals::withdrawals,
    },
    funds::FundsAmount,
};
use algonaut::{algod::v2::Algod, core::Address, indexer::v2::Indexer};
use anyhow::Result;
use chrono::{DateTime, Utc};

use super::received_payments::{received_payments, Payment};

#[derive(Debug, Clone)]
pub struct FundsActivityEntry {
    pub date: DateTime<Utc>,
    pub type_: FundsActivityEntryType,
    pub description: String,
    pub amount: FundsAmount,
    pub tx_id: TxId,
}

#[derive(Debug, Clone)]
pub enum FundsActivityEntryType {
    Income,
    Spending,
}

#[allow(clippy::too_many_arguments)]
pub async fn funds_activity(
    algod: &Algod,
    indexer: &Indexer,
    creator: &Address,
    dao_id: DaoId,
    customer_escrow_address: &Address,
    central_escrow_address: &Address,
    escrows: &Escrows,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<FundsActivityEntry>> {
    let withdrawals = withdrawals(algod, indexer, creator, dao_id, escrows, capi_deps).await?;
    // payments to the customer escrow
    let customer_escrow_payments = received_payments(indexer, customer_escrow_address).await?;
    // payments to the central escrow (either from investors buying shares, draining from customer escrow, or unexpected/not supported by the app payments)
    let central_escrow_payments = received_payments(indexer, central_escrow_address).await?;
    // filter out draining (payments from customer escrow to central escrow), which would duplicate payments to the customer escrow
    let filtered_central_escrow_payments: Vec<Payment> = central_escrow_payments
        .into_iter()
        .filter(|p| &p.sender != customer_escrow_address)
        .collect();

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

    for payment in customer_escrow_payments {
        funds_activity.push(FundsActivityEntry {
            date: payment.date,
            type_: FundsActivityEntryType::Income,
            description: payment
                .note
                .unwrap_or_else(|| "No description provided".to_owned()),
            amount: payment.amount,
            tx_id: payment.tx_id.clone(),
        })
    }

    for payment in filtered_central_escrow_payments {
        funds_activity.push(FundsActivityEntry {
            date: payment.date,
            type_: FundsActivityEntryType::Income,
            description: payment
                .note
                .unwrap_or_else(|| "No description provided".to_owned()),
            amount: payment.amount,
            tx_id: payment.tx_id.clone(),
        })
    }

    // sort ascendingly by date
    funds_activity.sort_by(|p1, p2| p1.date.cmp(&p2.date));

    Ok(funds_activity)
}

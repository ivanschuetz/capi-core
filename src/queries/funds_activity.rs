use super::received_payments::received_payments;
use crate::flows::withdraw::withdrawals::withdrawals;
use algonaut::{algod::v2::Algod, core::Address, indexer::v2::Indexer};
use anyhow::Result;
use chrono::{DateTime, Utc};
use mbase::models::{
    dao_id::DaoId,
    funds::{FundsAmount, FundsAssetId}, tx_id::TxId, capi_deps::CapiAssetDaoDeps,
};

#[derive(Debug, Clone)]
pub struct FundsActivityEntry {
    pub date: DateTime<Utc>,
    pub type_: FundsActivityEntryType,
    pub description: String,
    pub amount: FundsAmount,
    pub tx_id: TxId,
    pub address: Address,
    pub fee: FundsAmount,
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
    dao_id: DaoId,
    capi_deps: &CapiAssetDaoDeps,
    funds_asset: FundsAssetId,
) -> Result<Vec<FundsActivityEntry>> {
    let withdrawals = withdrawals(algod, indexer, dao_id, funds_asset, &None, &None).await?;
    // payments to the app escrow
    let payments = received_payments(
        indexer,
        &dao_id.0.address(),
        funds_asset,
        &None,
        &None,
        capi_deps,
    )
    .await?;

    let mut funds_activity = vec![];

    for withdrawal in withdrawals {
        funds_activity.push(FundsActivityEntry {
            date: withdrawal.date,
            type_: FundsActivityEntryType::Spending,
            description: withdrawal.description,
            amount: withdrawal.amount,
            tx_id: withdrawal.tx_id.clone(),
            address: withdrawal.address,
            fee: FundsAmount::new(0),
        })
    }

    for payment in payments {
        funds_activity.push(FundsActivityEntry {
            date: payment.date,
            type_: FundsActivityEntryType::Income,
            description: payment
                .note
                .unwrap_or_else(|| "No description provided".to_owned()),
            amount: payment.amount,
            tx_id: payment.tx_id.clone(),
            address: payment.sender,
            fee: payment.fee,
        })
    }

    // sort ascendingly by date
    funds_activity.sort_by(|p1, p2| p1.date.cmp(&p2.date));

    Ok(funds_activity)
}

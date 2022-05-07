use super::received_payments::all_received_payments;
use crate::{
    api::teal_api::TealApi,
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    flows::{
        create_dao::storage::load_dao::{DaoId, TxId},
        withdraw::withdrawals::withdrawals,
    },
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{algod::v2::Algod, core::Address, indexer::v2::Indexer};
use anyhow::Result;
use chrono::{DateTime, Utc};

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
    owner: &Address,
    dao_id: DaoId,
    customer_escrow_address: &Address,
    api: &dyn TealApi,
    capi_deps: &CapiAssetDaoDeps,
    funds_asset: FundsAssetId,
) -> Result<Vec<FundsActivityEntry>> {
    let withdrawals = withdrawals(
        algod,
        indexer,
        owner,
        dao_id,
        api,
        funds_asset,
        capi_deps,
        &None,
    )
    .await?;
    // payments to the customer escrow
    let payments = all_received_payments(
        indexer,
        &dao_id.0.address(),
        customer_escrow_address,
        funds_asset,
        &None,
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
        })
    }

    // sort ascendingly by date
    funds_activity.sort_by(|p1, p2| p1.date.cmp(&p2.date));

    Ok(funds_activity)
}

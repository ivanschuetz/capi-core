use crate::{
    capi_deps::{CapiAddress, CapiAssetDaoDeps},
    flows::create_dao::storage::load_dao::TxId,
    state::account_state::funds_holdings,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};
use mbase::{
    models::{
        dao_app_id::DaoAppId,
        funds::{FundsAmount, FundsAssetId},
        shares_percentage::SharesPercentage,
    },
    state::dao_app_state::dao_global_state,
};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

#[allow(clippy::too_many_arguments)]
pub async fn drain(
    algod: &Algod,
    drainer: &Address,
    app_id: DaoAppId,
    funds_asset_id: FundsAssetId,
    capi_deps: &CapiAssetDaoDeps,
    amounts: &DaoAndCapiDrainAmounts,
) -> Result<DrainToSign> {
    log::debug!("Will create drain txs, amounts: {amounts:?}");

    let params = algod.suggested_transaction_params().await?;

    let mut app_call_tx =
        drain_app_call_tx(app_id, &params, drainer, &capi_deps.address, funds_asset_id)?;

    // pay for the capi fee inner tx
    app_call_tx.fee = app_call_tx.fee * 2;

    Ok(DrainToSign { app_call_tx })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaoAndCapiDrainAmounts {
    // Part that goes to the dao (amount - fee)
    pub dao: FundsAmount,
    // Part that goes to capi (fee)
    pub capi: FundsAmount,
}

impl DaoAndCapiDrainAmounts {
    pub fn has_something_to_drain(&self) -> bool {
        // capi is just the fee (derived from the dao amount),
        // if the dao amount is 0, it means there's nothing to drain
        self.dao.val() > 0
    }
}

pub async fn to_drain_amounts(
    algod: &Algod,
    capi_percentage: SharesPercentage,
    funds_asset_id: FundsAssetId,
    app_id: DaoAppId,
) -> Result<DaoAndCapiDrainAmounts> {
    let dao_holdings = funds_holdings(algod, &app_id.address(), funds_asset_id).await?;
    let state = dao_global_state(algod, app_id).await?;

    let not_yet_drained = FundsAmount::new(
        dao_holdings
            .val()
            .checked_sub(state.available.val())
            .ok_or_else(|| {
                anyhow!(
                    "Error subtracting dao holdings: {dao_holdings:?} - {:?}",
                    state.available
                )
            })?,
    );

    let calc = calculate_dao_and_capi_escrow_xfer_amounts(not_yet_drained, capi_percentage)?;
    Ok(calc)
}

pub async fn fetch_drain_amount_and_drain(
    algod: &Algod,
    drainer: &Address,
    app_id: DaoAppId,
    funds_asset_id: FundsAssetId,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<DrainToSign> {
    let amounts =
        to_drain_amounts(algod, capi_deps.escrow_percentage, funds_asset_id, app_id).await?;

    drain(algod, drainer, app_id, funds_asset_id, capi_deps, &amounts).await
}

/// Note: always use this function to calculate fee, to prevent possible rounding mismatches.
pub fn calculate_dao_and_capi_escrow_xfer_amounts(
    amount_to_drain: FundsAmount,
    capi_percentage: SharesPercentage,
) -> Result<DaoAndCapiDrainAmounts> {
    // Note floor: to match TEAL truncated division https://developer.algorand.org/docs/get-details/dapps/avm/teal/opcodes/#_2
    // TODO checked arithmetic
    let capi_fee_amount =
        (amount_to_drain.as_decimal() * capi_percentage.value()).floor().to_u64().ok_or_else(|| anyhow!(
            "Invalid state: amount_for_capi_holders was floored and should be always convertible to u64"
        ))?;

    Ok(DaoAndCapiDrainAmounts {
        dao: FundsAmount::new(amount_to_drain.val() - capi_fee_amount),
        capi: FundsAmount::new(capi_fee_amount),
    })
}

pub fn drain_app_call_tx(
    app_id: DaoAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
    capi_address: &CapiAddress,
    funds_asset_id: FundsAssetId,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .app_arguments(vec!["drain".as_bytes().to_vec()])
            .foreign_assets(vec![funds_asset_id.0])
            .accounts(vec![capi_address.0])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_drain(algod: &Algod, signed: &DrainSigned) -> Result<TxId> {
    log::debug!("calling submit drain..");

    // mbase::teal::debug_teal_rendered(&[signed.app_call_tx_signed.clone()], "dao_app_approval")
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[signed.app_call_tx_signed.clone()])
        .await?;
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainToSign {
    pub app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainSigned {
    pub app_call_tx_signed: SignedTransaction,
}

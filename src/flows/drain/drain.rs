use crate::{
    algo_helpers::calculate_total_fee,
    capi_asset::{capi_app_id::CapiAppId, capi_asset_dao_specs::CapiAssetDaoDeps},
    flows::create_project::{shares_percentage::SharesPercentage, storage::load_project::TxId},
    funds::{FundsAmount, FundsAssetId},
    state::account_state::funds_holdings,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{
        builder::{CallApplication, TxnFee},
        contract_account::ContractAccount,
        tx_group::TxGroup,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::{anyhow, Result};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

pub async fn drain_customer_escrow(
    algod: &Algod,
    drainer: &Address,
    central_app_id: u64,
    funds_asset_id: FundsAssetId,
    capi_deps: &CapiAssetDaoDeps,
    customer_escrow: &ContractAccount,
    central_escrow: &ContractAccount,
    amounts: &DaoAndCapiDrainAmounts,
) -> Result<DrainCustomerEscrowToSign> {
    let params = algod.suggested_transaction_params().await?;

    let app_call_tx = &mut drain_app_call_tx(central_app_id, &params, drainer)?;
    let capi_app_call_tx = &mut drain_capi_app_call_tx(capi_deps.app_id, &params, drainer)?;

    let drain_tx = &mut TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *customer_escrow.address(),
            funds_asset_id.0,
            amounts.dao.val(),
            *central_escrow.address(),
        )
        .build(),
    )
    .build()?;

    let capi_share_tx = &mut TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *customer_escrow.address(),
            funds_asset_id.0,
            amounts.capi.val(),
            capi_deps.escrow,
        )
        .build(),
    )
    .build()?;

    app_call_tx.fee = calculate_total_fee(&params, &[app_call_tx, drain_tx, capi_share_tx])?;
    TxGroup::assign_group_id(vec![app_call_tx, capi_app_call_tx, drain_tx, capi_share_tx])?;

    let signed_drain_tx = customer_escrow.sign(drain_tx, vec![])?;
    let signed_capi_share_tx = customer_escrow.sign(capi_share_tx, vec![])?;

    Ok(DrainCustomerEscrowToSign {
        drain_tx: signed_drain_tx,
        capi_share_tx: signed_capi_share_tx,
        app_call_tx: app_call_tx.clone(),
        capi_app_call_tx: capi_app_call_tx.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaoAndCapiDrainAmounts {
    pub dao: FundsAmount,
    pub capi: FundsAmount,
}

pub async fn drain_amounts(
    algod: &Algod,
    capi_percentage: SharesPercentage,
    funds_asset_id: FundsAssetId,
    customer_escrow: &Address,
) -> Result<DaoAndCapiDrainAmounts> {
    let customer_escrow_holdings = funds_holdings(algod, customer_escrow, funds_asset_id).await?;
    calculate_dao_and_capi_escrow_xfer_amounts(customer_escrow_holdings, capi_percentage.clone())
}

fn calculate_dao_and_capi_escrow_xfer_amounts(
    amount_to_drain: FundsAmount,
    capi_percentage: SharesPercentage,
) -> Result<DaoAndCapiDrainAmounts> {
    // Take cut for $capi holders. Note rounding: it will be variably favorable to DAO or Capi investors.
    let amount_for_capi_holders =
        (amount_to_drain.as_decimal() * capi_percentage.value()).round().to_u64().ok_or(anyhow!(
            "Invalid state: amount_for_capi_holders was rounded and should be always convertible to u64"
        ))?;

    Ok(DaoAndCapiDrainAmounts {
        dao: FundsAmount::new(amount_to_drain.val() - amount_for_capi_holders),
        capi: FundsAmount::new(amount_for_capi_holders),
    })
}

pub fn drain_app_call_tx(
    app_id: u64,
    params: &SuggestedTransactionParams,
    sender: &Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(params, CallApplication::new(*sender, app_id).build()).build()?;
    Ok(tx)
}

pub fn drain_capi_app_call_tx(
    app_id: CapiAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(params, CallApplication::new(*sender, app_id.0).build()).build()?;
    Ok(tx)
}

pub async fn submit_drain_customer_escrow(
    algod: &Algod,
    signed: &DrainCustomerEscrowSigned,
) -> Result<TxId> {
    log::debug!("calling submit drain..");

    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.capi_app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.capi_share_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();

    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.capi_app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.capi_share_tx.clone(),
    //     ],
    //     "app_capi_approval",
    // )
    // .unwrap();

    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.capi_app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.capi_share_tx.clone(),
    //     ],
    //     "customer_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.app_call_tx_signed.clone(),
            signed.capi_app_call_tx_signed.clone(),
            signed.drain_tx.clone(),
            signed.capi_share_tx.clone(),
        ])
        .await?;
    log::debug!("Drain customer escrow tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainCustomerEscrowToSign {
    pub drain_tx: SignedTransaction,
    pub capi_share_tx: SignedTransaction,
    pub capi_app_call_tx: Transaction,
    pub app_call_tx: Transaction,
    // pub amount_to_drain: FundsAmount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainCustomerEscrowSigned {
    pub drain_tx: SignedTransaction,
    pub capi_share_tx: SignedTransaction,
    pub capi_app_call_tx_signed: SignedTransaction,
    pub app_call_tx_signed: SignedTransaction,
}

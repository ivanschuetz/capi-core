use crate::{
    algo_helpers::calculate_total_fee, capi_deps::CapiAssetDaoDeps,
    flows::create_dao::storage::load_dao::TxId, state::account_state::funds_holdings,
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
use mbase::models::{
    dao_app_id::DaoAppId,
    funds::{FundsAmount, FundsAssetId},
    shares_percentage::SharesPercentage,
};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

#[allow(clippy::too_many_arguments)]
pub async fn drain_customer_escrow(
    algod: &Algod,
    drainer: &Address,
    app_id: DaoAppId,
    funds_asset_id: FundsAssetId,
    capi_deps: &CapiAssetDaoDeps,
    customer_escrow: &ContractAccount,
    amounts: &DaoAndCapiDrainAmounts,
) -> Result<DrainCustomerEscrowToSign> {
    log::debug!("Will create drain txs, amounts: {amounts:?}");

    let params = algod.suggested_transaction_params().await?;

    let mut app_call_tx = drain_app_call_tx(
        app_id,
        &params,
        drainer,
        customer_escrow.address(),
        funds_asset_id,
    )?;

    let mut drain_tx = TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *customer_escrow.address(),
            funds_asset_id.0,
            amounts.dao.val(),
            app_id.address(),
        )
        .build(),
    )
    .build()?;

    let mut capi_share_tx = TxnBuilder::with_fee(
        &params,
        TxnFee::zero(),
        TransferAsset::new(
            *customer_escrow.address(),
            funds_asset_id.0,
            amounts.capi.val(),
            capi_deps.address.0,
        )
        .build(),
    )
    .build()?;

    app_call_tx.fee = calculate_total_fee(&params, &[&app_call_tx, &drain_tx, &capi_share_tx])?;
    TxGroup::assign_group_id(&mut [&mut app_call_tx, &mut drain_tx, &mut capi_share_tx])?;

    let signed_drain_tx = customer_escrow.sign(drain_tx, vec![])?;
    let signed_capi_share_tx = customer_escrow.sign(capi_share_tx, vec![])?;

    Ok(DrainCustomerEscrowToSign {
        drain_tx: signed_drain_tx,
        capi_share_tx: signed_capi_share_tx,
        app_call_tx: app_call_tx.clone(),
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
    calculate_dao_and_capi_escrow_xfer_amounts(customer_escrow_holdings, capi_percentage)
}

pub async fn fetch_drain_amount_and_drain(
    algod: &Algod,
    drainer: &Address,
    app_id: DaoAppId,
    funds_asset_id: FundsAssetId,
    capi_deps: &CapiAssetDaoDeps,
    customer_escrow: &ContractAccount,
) -> Result<DrainCustomerEscrowToSign> {
    let amounts = drain_amounts(
        algod,
        capi_deps.escrow_percentage,
        funds_asset_id,
        customer_escrow.address(),
    )
    .await?;

    drain_customer_escrow(
        algod,
        drainer,
        app_id,
        funds_asset_id,
        capi_deps,
        customer_escrow,
        &amounts,
    )
    .await
}

fn calculate_dao_and_capi_escrow_xfer_amounts(
    amount_to_drain: FundsAmount,
    capi_percentage: SharesPercentage,
) -> Result<DaoAndCapiDrainAmounts> {
    // Take cut for $capi holders. Note floor: to match TEAL truncated division https://developer.algorand.org/docs/get-details/dapps/avm/teal/opcodes/#_2
    let amount_for_capi_holders =
        (amount_to_drain.as_decimal() * capi_percentage.value()).floor().to_u64().ok_or(anyhow!(
            "Invalid state: amount_for_capi_holders was floored and should be always convertible to u64"
        ))?;

    Ok(DaoAndCapiDrainAmounts {
        dao: FundsAmount::new(amount_to_drain.val() - amount_for_capi_holders),
        capi: FundsAmount::new(amount_for_capi_holders),
    })
}

pub fn drain_app_call_tx(
    app_id: DaoAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
    customer_escrow: &Address,
    funds_asset_id: FundsAssetId,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .app_arguments(vec!["drain".as_bytes().to_vec()])
            .foreign_assets(vec![funds_asset_id.0])
            .accounts(vec![*customer_escrow])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_drain_customer_escrow(
    algod: &Algod,
    signed: &DrainCustomerEscrowSigned,
) -> Result<TxId> {
    log::debug!("calling submit drain..");

    // mbase::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.capi_share_tx.clone(),
    //     ],
    //     "dao_app_approval",
    // )
    // .unwrap();

    // mbase::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.capi_share_tx.clone(),
    //     ],
    //     "customer_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.app_call_tx_signed.clone(),
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
    pub app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainCustomerEscrowSigned {
    pub drain_tx: SignedTransaction,
    pub capi_share_tx: SignedTransaction,
    pub app_call_tx_signed: SignedTransaction,
}

use crate::{
    capi_asset::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetAmount},
    decimal_util::AsDecimal,
    flows::create_project::storage::load_project::TxId,
    funds::{FundsAmount, FundsAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, contract_account::ContractAccount, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn harvest(
    algod: &Algod,
    harvester: &Address,
    capi_app_id: CapiAppId,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
    capi_escrow: &ContractAccount,
) -> Result<HarvestToSign> {
    log::debug!("Generating capi harvest txs, harvester: {:?}, capi_app_id: {:?}, amount: {:?}, central_escrow: {:?}", harvester, capi_app_id, amount, capi_escrow);
    let params = algod.suggested_transaction_params().await?;

    // App call to update user's local state with harvested amount
    let app_call_tx = &mut harvest_app_call_tx(capi_app_id, &params, harvester)?;

    // The harvester pays the fee of the harvest tx (signed by central escrow)
    let pay_fee_tx = &mut TxnBuilder::with(
        &params,
        Pay::new(
            *harvester,
            *capi_escrow.address(),
            params.fee.max(params.min_fee),
        )
        .build(),
    )
    .build()?;

    // Funds transfer from escrow to creator
    let harvest_tx = &mut TxnBuilder::with(
        &params,
        TransferAsset::new(
            *capi_escrow.address(),
            funds_asset_id.0,
            amount.val(),
            *harvester,
        )
        .build(),
    )
    .build()?;

    TxGroup::assign_group_id(vec![app_call_tx, pay_fee_tx, harvest_tx])?;

    let signed_harvest_tx = capi_escrow.sign(harvest_tx, vec![])?;

    Ok(HarvestToSign {
        app_call_tx: app_call_tx.clone(),
        harvest_tx: signed_harvest_tx,
        pay_fee_tx: pay_fee_tx.clone(),
    })
}

pub fn harvest_app_call_tx(
    app_id: CapiAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .app_arguments(vec!["harvest".as_bytes().to_vec()])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_harvest(algod: &Algod, signed: &HarvestSigned) -> Result<TxId> {
    log::debug!("Submit capi harvest..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.app_call_tx_signed.clone(),
        signed.pay_fee_tx.clone(),
        signed.harvest_tx.clone(),
    ];
    // crate::teal::debug_teal_rendered(&txs, "app_capi_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Harvest tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

pub fn calculate_capi_entitled_harvest(
    received_total: FundsAmount,
    share_supply: CapiAssetAmount,
    share_count: CapiAssetAmount,
    precision: u64,
) -> FundsAmount {
    // TODO review possible overflow, type cast, unwrap
    // for easier understanding we use the same arithmetic as in TEAL
    let entitled_percentage =
        ((share_count.val() * precision).as_decimal() / share_supply.as_decimal()).floor();
    let entitled_total =
        ((received_total.as_decimal() * entitled_percentage) / (precision.as_decimal())).floor();

    FundsAmount::new(entitled_total.to_u64().unwrap())
}

pub fn investor_can_harvest_amount_capi_calc(
    central_received_total: FundsAmount,
    harvested_total: FundsAmount,
    share_amount: CapiAssetAmount,
    share_supply: CapiAssetAmount,
    precision: u64,
) -> FundsAmount {
    // Note that this assumes that investor can't unlock only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with harvested_total, which remains unchanged.
    // the easiest solution is to expect the investor to unlock all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.

    let entitled_total = calculate_capi_entitled_harvest(
        central_received_total,
        share_supply,
        share_amount,
        precision,
    );
    entitled_total - harvested_total
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestToSign {
    pub app_call_tx: Transaction,
    pub harvest_tx: SignedTransaction,
    pub pay_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarvestSigned {
    pub app_call_tx_signed: SignedTransaction,
    pub harvest_tx: SignedTransaction,
    pub pay_fee_tx: SignedTransaction,
}

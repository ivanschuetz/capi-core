use crate::{
    capi_asset::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetId},
    flows::create_dao::storage::load_dao::TxId,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{builder::CloseApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn unlock_capi_assets(
    algod: &Algod,
    investor: &Address,
    app_id: CapiAppId,
    asset_id: CapiAssetId,
) -> Result<UnlockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to validate the retrieved shares count and clear local state
    let mut app_call = TxnBuilder::with(
        &params,
        CloseApplication::new(*investor, app_id.0)
            .app_arguments(vec!["unlock".as_bytes().to_vec()])
            .foreign_assets(vec![asset_id.0])
            .build(),
    )
    .build()?;

    // pay for inner tx capi asset xfer
    app_call.fee = app_call.fee * 2;

    Ok(UnlockToSign {
        capi_app_optout_tx: app_call.clone(),
    })
}

pub async fn submit_capi_assets_unlock(algod: &Algod, signed: UnlockSigned) -> Result<TxId> {
    log::debug!("Submit capi asset unlock..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.capi_app_optout_tx];

    // crate::teal::debug_teal_rendered(&txs, "capi_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "capi_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockToSign {
    pub capi_app_optout_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockSigned {
    pub capi_app_optout_tx: SignedTransaction,
}

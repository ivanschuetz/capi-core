use crate::flows::create_dao::storage::load_dao::{DaoAppId, TxId};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{builder::CloseApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn unlock(
    algod: &Algod,
    investor: Address,
    app_id: DaoAppId,
    shares_asset_id: u64,
) -> Result<UnlockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to validate the retrieved shares count and clear local state
    let mut central_app_optout_tx = TxnBuilder::with(
        &params,
        CloseApplication::new(investor, app_id.0)
            .app_arguments(vec!["unlock".as_bytes().to_vec()])
            .foreign_assets(vec![shares_asset_id])
            .build(),
    )
    .build()?;

    // pay for the xfer inner tx
    central_app_optout_tx.fee = central_app_optout_tx.fee * 2;

    Ok(UnlockToSign {
        central_app_optout_tx: central_app_optout_tx.clone(),
    })
}

pub async fn submit_unlock(algod: &Algod, signed: UnlockSigned) -> Result<TxId> {
    log::debug!("calling submit unlock..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.central_app_optout_tx];

    // crate::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockToSign {
    pub central_app_optout_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockSigned {
    pub central_app_optout_tx: SignedTransaction,
}

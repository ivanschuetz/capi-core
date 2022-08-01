use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::{models::tx_id::TxId, util::network_util::wait_for_pending_transaction};
use serde::{Deserialize, Serialize};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn rekey(algod: &Algod, to_rekey: &Address, auth: &Address) -> Result<RekeyToSign> {
    let params = algod.suggested_transaction_params().await?;
    log::debug!("Creating rekey txs, from: {to_rekey:?} to: {auth:?}");

    let tx = TxnBuilder::with(
        &params,
        Pay::new(*to_rekey, *to_rekey, MicroAlgos(0)).build(),
    )
    .rekey_to(*auth)
    .build()?;

    log::debug!("create rekey tx: {tx:?}");

    Ok(RekeyToSign { tx })
}

pub async fn submit_rekey(algod: &Algod, signed: RekeySigned) -> Result<TxId> {
    log::debug!("calling submit rekey..");
    log::debug!("submit rekey tx: {signed:?}");

    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.tx];

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Rekey tx id: {:?}", res.tx_id);

    let _ = wait_for_pending_transaction(algod, &res.tx_id.parse()?).await?;

    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekeyToSign {
    pub tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RekeySigned {
    pub tx: SignedTransaction,
}

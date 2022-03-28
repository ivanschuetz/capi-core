use crate::{
    flows::create_dao::storage::load_dao::{DaoAppId, TxId},
    teal::TealSource,
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{builder::UpdateApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub async fn update(
    algod: &Algod,
    owner: &Address,
    app_id: DaoAppId,
    approval: TealSource,
    clear: TealSource,
) -> Result<UpdateAppToSign> {
    let params = algod.suggested_transaction_params().await?;

    let compiled_approval = algod.compile_teal(&approval.0).await?;
    let compiled_clear = algod.compile_teal(&clear.0).await?;

    let update = TxnBuilder::with(
        &params,
        UpdateApplication::new(*owner, app_id.0, compiled_approval, compiled_clear).build(),
    )
    .build()?;

    Ok(UpdateAppToSign { update })
}

pub async fn submit_update(algod: &Algod, signed: UpdateAppSigned) -> Result<TxId> {
    log::debug!("calling submit app update..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.update];

    // crate::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAppToSign {
    pub update: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateAppSigned {
    pub update: SignedTransaction,
}

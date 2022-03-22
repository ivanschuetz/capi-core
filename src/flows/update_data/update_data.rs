use crate::flows::create_dao::storage::load_dao::{DaoAppId, TxId};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Dao app data that is meant to be updated externally
#[derive(Debug, Clone)]
pub struct UpdatableDaoData {
    pub central_escrow: Address,
    pub customer_escrow: Address,
}

pub async fn update_data(
    algod: &Algod,
    owner: &Address,
    app_id: DaoAppId,
    data: &UpdatableDaoData,
) -> Result<UpdateAppToSign> {
    let params = algod.suggested_transaction_params().await?;

    // We might make these updates more granular later. For now everything in 1 call.
    let update = TxnBuilder::with(
        &params,
        CallApplication::new(*owner, app_id.0)
            .app_arguments(vec![
                "update_data".as_bytes().to_vec(),
                data.central_escrow.0.to_vec(),
                data.customer_escrow.0.to_vec(),
            ])
            .build(),
    )
    .build()?;

    Ok(UpdateAppToSign { update })
}

pub async fn submit_unlock(algod: &Algod, signed: UpdateAppSigned) -> Result<TxId> {
    log::debug!("calling submit app data update..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.update];

    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();

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

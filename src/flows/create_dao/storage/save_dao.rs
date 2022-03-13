use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;

use crate::{flows::create_dao::model::Dao, hashable::Hashable};

use super::{load_dao::TxId, note::dao_to_note};

pub async fn save_dao(algod: &Algod, creator: &Address, dao: &Dao) -> Result<SaveDaoToSign> {
    let params = algod.suggested_transaction_params().await?;

    let note = dao_to_note(dao)?;
    // log::debug!("Note bytes: {:?}", note.len());

    let tx = TxnBuilder::with(&params, Pay::new(*creator, *creator, MicroAlgos(0)).build())
        .note(note)
        .build()?;

    Ok(SaveDaoToSign {
        tx,
        dao: dao.to_owned(),
    })
}

impl Hashable for Dao {}

pub async fn submit_save_dao(algod: &Algod, signed: SaveDaoSigned) -> Result<TxId> {
    let res = algod.broadcast_signed_transaction(&signed.tx).await?;
    log::debug!("Save dao tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDaoToSign {
    pub tx: Transaction,
    pub dao: Dao,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDaoSigned {
    pub tx: SignedTransaction,
}

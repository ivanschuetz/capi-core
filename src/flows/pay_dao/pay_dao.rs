use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{SignedTransaction, Transaction, TransferAsset, TxnBuilder},
};
use anyhow::Result;
use mbase::models::{
    dao_app_id::DaoAppId,
    funds::{FundsAmount, FundsAssetId}, tx_id::TxId,
};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

pub async fn pay_dao_app(
    algod: &Algod,
    customer: &Address,
    app_id: DaoAppId,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
) -> Result<PayDaoToSign> {
    let params = algod.suggested_transaction_params().await?;

    let tx = TxnBuilder::with(
        &params,
        TransferAsset::new(*customer, funds_asset_id.0, amount.val(), app_id.address()).build(),
    )
    .build()?;

    Ok(PayDaoToSign { tx })
}

pub async fn submit_pay_dao(algod: &Algod, signed: PayDaoSigned) -> Result<TxId> {
    let res = algod.broadcast_signed_transaction(&signed.tx).await?;
    log::debug!("Pay dao tx id: {:?}", res.tx_id);
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayDaoToSign {
    pub tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayDaoSigned {
    pub tx: SignedTransaction,
}

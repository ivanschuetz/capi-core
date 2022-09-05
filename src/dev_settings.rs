use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::models::{dao_app_id::DaoAppId, timestamp::Timestamp, tx_id::TxId};
use serde::{Deserialize, Serialize};

#[allow(clippy::too_many_arguments)]
pub async fn dev_settings(
    algod: &Algod,
    sender: &Address,
    app_id: DaoAppId,
    settings: &DevSettings,
) -> Result<DevSettingsToSign> {
    log::debug!("Will create dev settings txs with settings: {settings:?}");

    let params = algod.suggested_transaction_params().await?;

    let app_call_tx = dev_settings_app_call_tx(app_id, &params, sender, &settings)?;

    Ok(DevSettingsToSign { app_call_tx })
}

#[derive(Debug, Clone)]
pub struct DevSettings {
    pub min_raise_target_end_date: Timestamp,
}

pub fn dev_settings_app_call_tx(
    app_id: DaoAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
    settings: &DevSettings,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .app_arguments(vec![
                "dev_settings".as_bytes().to_vec(),
                settings.min_raise_target_end_date.0.to_be_bytes().to_vec(),
            ])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_dev_settings(algod: &Algod, signed: &DevSettingsSigned) -> Result<TxId> {
    log::debug!("calling submit dev settings..");

    let res = algod
        .broadcast_signed_transactions(&[signed.app_call_tx.clone()])
        .await?;
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevSettingsToSign {
    pub app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevSettingsSigned {
    pub app_call_tx: SignedTransaction,
}

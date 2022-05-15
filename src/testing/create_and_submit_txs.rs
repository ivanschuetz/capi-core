#[cfg(test)]
pub use test::{pay_submit, transfer_tokens_submit};

// need wrapper module for auto imports to work https://github.com/rust-analyzer/rust-analyzer/issues/9391
#[cfg(test)]
mod test {
    use crate::algo_helpers::send_tx_and_wait;
    use algonaut::{
        algod::v2::Algod,
        core::{Address, MicroAlgos, SuggestedTransactionParams},
        transaction::{account::Account, Pay, TransferAsset, TxnBuilder},
    };
    use anyhow::Result;
    use mbase::models::asset_amount::AssetAmount;

    #[allow(dead_code)]
    pub async fn pay_submit(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        sender: &Account,
        receiver: &Address,
        amount: MicroAlgos,
    ) -> Result<()> {
        let tx = TxnBuilder::with(
            &params,
            Pay::new(sender.address(), *receiver, amount).build(),
        )
        .build()?;
        let signed = sender.sign_transaction(tx)?;
        log::debug!("Submitting payment");
        send_tx_and_wait(&algod, &signed).await?;
        Ok(())
    }

    pub async fn transfer_tokens_submit(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        sender: &Account,
        receiver: &Address,
        asset_id: u64,
        amount: AssetAmount,
    ) -> Result<()> {
        let tx = TxnBuilder::with(
            &params,
            TransferAsset::new(sender.address(), asset_id, amount.0, *receiver).build(),
        )
        .build()?;
        let signed = sender.sign_transaction(tx)?;
        log::debug!("Submitting xfer");
        send_tx_and_wait(&algod, &signed).await?;
        Ok(())
    }
}

#[cfg(test)]
pub use test::{
    optin_to_app_submit, optin_to_asset_submit, transfer_tokens_and_pay_fee_submit,
    transfer_tokens_submit,
};

// need wrapper module for auto imports to work https://github.com/rust-analyzer/rust-analyzer/issues/9391
#[cfg(test)]
mod test {

    use crate::{
        flows::shared::app::optin_to_app, network_util::wait_for_pending_transaction,
        testing::algorand_checks::optin_to_asset,
    };
    use algonaut::{
        algod::v2::Algod,
        core::{Address, SuggestedTransactionParams},
        transaction::{account::Account, tx_group::TxGroup, Pay, TransferAsset, TxnBuilder},
    };
    use anyhow::Result;

    /// Do an asset transfer and pay the fee for it - usually needed when the asset sender is an escrow
    /// Note that this is used only in some test txs (where we don't necessarily have a tx that can pay for the fee)
    pub async fn transfer_tokens_and_pay_fee_submit(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        fee_payer: &Account,
        xfer_sender: &Account,
        receiver: &Address,
        asset_id: u64,
        amount: u64,
    ) -> Result<()> {
        let mut xfer_tx = TxnBuilder::with(
            &params,
            TransferAsset::new(xfer_sender.address(), asset_id, amount, *receiver).build(),
        )
        .build()?;
        let mut pay_fee_tx = TxnBuilder::with(
            &params,
            Pay::new(
                fee_payer.address(),
                *receiver,
                xfer_tx.estimate_fee_with_params(params)?,
            )
            .build(),
        )
        .build()?;
        TxGroup::assign_group_id(vec![&mut pay_fee_tx, &mut xfer_tx])?;

        let signed_payment = fee_payer.sign_transaction(&pay_fee_tx)?;
        let signed_xfer = xfer_sender.sign_transaction(&xfer_tx)?;
        log::debug!("Submitting xfer and pay for fee");
        let res = algod
            .broadcast_signed_transactions(&[signed_payment, signed_xfer])
            .await?;
        wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;
        Ok(())
    }

    pub async fn transfer_tokens_submit(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        sender: &Account,
        receiver: &Address,
        asset_id: u64,
        amount: u64,
    ) -> Result<()> {
        let tx = TxnBuilder::with(
            &params,
            TransferAsset::new(sender.address(), asset_id, amount, *receiver).build(),
        )
        .build()?;
        let signed = sender.sign_transaction(&tx)?;
        log::debug!("Submitting xfer");
        let res = algod.broadcast_signed_transaction(&signed).await?;
        wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;
        Ok(())
    }

    pub async fn optin_to_asset_submit(
        algod: &Algod,
        sender: &Account,
        asset_id: u64,
    ) -> Result<()> {
        let tx = optin_to_asset(&algod, &sender.address(), asset_id).await?;
        let signed = sender.sign_transaction(&tx)?;
        log::debug!("Submitting asset opt in: {asset_id}");
        let res = algod.broadcast_signed_transaction(&signed).await?;
        wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;
        Ok(())
    }

    pub async fn optin_to_app_submit(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        sender: &Account,
        app_id: u64,
    ) -> Result<()> {
        let tx = optin_to_app(params, app_id, sender.address()).await?;
        let signed = sender.sign_transaction(&tx)?;
        log::debug!("Submitting app opt in: {app_id}");
        let res = algod.broadcast_signed_transaction(&signed).await?;
        wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;
        Ok(())
    }
}

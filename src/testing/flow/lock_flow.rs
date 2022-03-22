#[cfg(test)]
pub use test::lock_flow;

#[cfg(test)]
pub mod test {
    use crate::flows::create_dao::model::Dao;
    use crate::flows::create_dao::share_amount::ShareAmount;
    use crate::flows::lock::lock::{lock, submit_lock, LockSigned};
    use crate::network_util::wait_for_pending_transaction;
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::Result;

    pub async fn lock_flow(
        algod: &Algod,
        dao: &Dao,
        investor: &Account,
        amount: ShareAmount,
    ) -> Result<()> {
        let lock_to_sign = lock(
            &algod,
            investor.address(),
            amount,
            dao.shares_asset_id,
            dao.app_id,
            &dao.locking_escrow,
            dao.id(),
        )
        .await?;

        let signed_app_call_tx =
            investor.sign_transaction(lock_to_sign.central_app_call_setup_tx)?;

        let signed_shares_xfer_tx = investor.sign_transaction(lock_to_sign.shares_xfer_tx)?;
        let tx_id = submit_lock(
            algod,
            LockSigned {
                central_app_call_setup_tx: signed_app_call_tx,
                shares_xfer_tx_signed: signed_shares_xfer_tx,
            },
        )
        .await?;
        let _ = wait_for_pending_transaction(&algod, &tx_id);

        Ok(())
    }
}

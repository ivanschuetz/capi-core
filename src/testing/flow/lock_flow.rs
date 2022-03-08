#[cfg(test)]
pub use test::lock_flow;

#[cfg(test)]
pub mod test {
    use crate::flows::create_project::model::Project;
    use crate::flows::create_project::{
        share_amount::ShareAmount, storage::load_project::ProjectId,
    };
    use crate::flows::lock::lock::{lock, submit_lock, LockSigned};
    use crate::network_util::wait_for_pending_transaction;
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::Result;

    pub async fn lock_flow(
        algod: &Algod,
        project: &Project,
        project_id: &ProjectId,
        investor: &Account,
        amount: ShareAmount,
    ) -> Result<()> {
        let lock_to_sign = lock(
            &algod,
            investor.address(),
            amount,
            project.shares_asset_id,
            project.central_app_id,
            &project.locking_escrow,
            project_id,
        )
        .await?;

        let signed_app_call_tx =
            investor.sign_transaction(&lock_to_sign.central_app_call_setup_tx)?;

        let signed_shares_xfer_tx = investor.sign_transaction(&lock_to_sign.shares_xfer_tx)?;
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

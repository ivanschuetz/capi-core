#[cfg(test)]
pub use test::lock_capi_asset_flow;

#[cfg(test)]
mod test {
    use crate::{
        capi_asset::{
            capi_app_id::CapiAppId,
            capi_asset_id::{CapiAssetAmount, CapiAssetId},
            lock::lock::{lock_capi_assets, submit_capi_assets_lock, LockSigned},
        },
        network_util::wait_for_pending_transaction,
    };
    use algonaut::{algod::v2::Algod, core::Address, transaction::account::Account};
    use anyhow::Result;

    pub async fn lock_capi_asset_flow(
        algod: &Algod,
        investor: &Account,
        amount: CapiAssetAmount,
        asset_id: CapiAssetId,
        app_id: CapiAppId,
        capi_escrow: &Address,
    ) -> Result<()> {
        let to_sign = lock_capi_assets(
            &algod,
            &investor.address(),
            amount,
            asset_id,
            app_id,
            capi_escrow,
        )
        .await?;
        let signed_app_call = investor.sign_transaction(to_sign.capi_app_call_setup_tx)?;
        let signed_xfer = investor.sign_transaction(to_sign.shares_xfer_tx)?;

        // crate::teal::debug_teal_rendered(
        //     &vec![signed_app_call.clone(), signed_xfer.clone()],
        //     "capi_app_approval",
        // )
        // .unwrap();

        let submit_lock_tx_id = submit_capi_assets_lock(
            &algod,
            LockSigned {
                capi_app_call_setup_tx: signed_app_call,
                shares_xfer_tx_signed: signed_xfer,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &submit_lock_tx_id).await?;

        Ok(())
    }
}

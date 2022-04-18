#[cfg(test)]
pub use test::unlock_capi_asset_flow;
#[cfg(test)]
mod test {
    use crate::{
        capi_asset::{
            capi_app_id::CapiAppId,
            capi_asset_id::CapiAssetId,
            unlock::unlock::{submit_capi_assets_unlock, unlock_capi_assets, UnlockSigned},
        },
        network_util::wait_for_pending_transaction,
    };
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::Result;

    pub async fn unlock_capi_asset_flow(
        algod: &Algod,
        investor: &Account,
        app_id: CapiAppId,
        asset_id: CapiAssetId,
    ) -> Result<()> {
        let to_sign = unlock_capi_assets(&algod, &investor.address(), app_id, asset_id).await?;
        let signed_app_opt_out = investor.sign_transaction(to_sign.capi_app_optout_tx)?;

        let submit_lock_tx_id = submit_capi_assets_unlock(
            &algod,
            UnlockSigned {
                capi_app_optout_tx: signed_app_opt_out,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &submit_lock_tx_id).await?;

        Ok(())
    }
}

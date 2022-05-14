#[cfg(test)]
pub use test::test_shares_locked;

#[cfg(test)]
pub mod test {
    use crate::{
        capi_asset::{
            capi_app_id::CapiAppId,
            capi_app_state::capi_app_investor_state_from_acc,
            capi_asset_id::{CapiAssetAmount, CapiAssetId},
        },
        state::account_state::{asset_holdings, find_asset_holding_or_err},
    };
    use algonaut::{algod::v2::Algod, core::Address};
    use anyhow::Result;
    use mbase::models::funds::FundsAmount;

    pub async fn test_shares_locked(
        algod: &Algod,
        investor: &Address,
        capi_asset_id: CapiAssetId,
        capi_app_id: CapiAppId,
        locked_amount: CapiAssetAmount,
        remaining_investor_amount: CapiAssetAmount,
        capi_escrow: &Address,
    ) -> Result<()> {
        let investor_infos = algod.account_information(investor).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let asset_holding = find_asset_holding_or_err(&investor_assets, capi_asset_id.0)?;
        assert_eq!(remaining_investor_amount.0, asset_holding.amount);

        // escrow got capi assets

        let app_escrow_infos = algod.account_information(capi_escrow).await?;
        let app_escrow_assets = app_escrow_infos.assets;
        assert_eq!(2, app_escrow_assets.len()); // opted in to shares and capi token
        let capi_asset_holdings = asset_holdings(&algod, capi_escrow, capi_asset_id.0).await?;
        assert_eq!(locked_amount, CapiAssetAmount::new(capi_asset_holdings.0));

        // local state is correct

        let investor_state = capi_app_investor_state_from_acc(&investor_infos, capi_app_id)?;
        // shares local state initialized to shares
        assert_eq!(locked_amount, investor_state.shares);
        // claimed total is initialized to entitled amount, which at this point is 0 because the escrow doesn't have any funds
        assert_eq!(FundsAmount::new(0), investor_state.claimed);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::api::api::LocalApi;
    use crate::capi_asset::capi_app_state::{capi_app_global_state, capi_app_investor_state};
    use crate::capi_asset::capi_asset_id::{CapiAssetAmount, CapiAssetId};
    use crate::capi_asset::create::test_flow::test_flow::setup_capi_asset_flow;
    use crate::funds::FundsAmount;
    use crate::testing::network_test_util::create_and_distribute_funds_asset;
    use crate::{
        dependencies,
        state::app_state::ApplicationLocalStateError,
        testing::{network_test_util::test_init, test_data::creator},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_create_capi_token_and_app() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let api = LocalApi {};
        let creator = creator();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        let capi_supply = CapiAssetAmount::new(1_000_000_000);

        // flow

        let flow_res =
            setup_capi_asset_flow(&algod, &api, &creator, capi_supply, funds_asset_id).await?;

        // tests

        let creator_infos = algod.account_information(&creator.address()).await?;
        let created_assets = creator_infos.created_assets;

        assert_eq!(created_assets.len(), 1);

        // created asset checks
        // assert_eq!(created_assets[0].params.creator, creator.address()); // TODO is this field optional or not
        assert_eq!(flow_res.asset_id, CapiAssetId(created_assets[0].index));
        assert_eq!(
            capi_supply,
            CapiAssetAmount::new(created_assets[0].params.total)
        );

        // The app hasn't received anything yet
        let app_global_state = capi_app_global_state(&algod, flow_res.app_id).await?;
        assert_eq!(FundsAmount::new(0), app_global_state.received);

        // The creator doesn't automatically opt in to the app
        let app_investor_state_res =
            capi_app_investor_state(&algod, &creator.address(), flow_res.app_id).await;
        assert_eq!(
            Err(ApplicationLocalStateError::NotOptedIn),
            app_investor_state_res
        );

        Ok(())
    }
}

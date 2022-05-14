#[cfg(test)]
mod tests {
    use crate::{
        api::teal_api::LocalTealApi,
        capi_asset::{
            capi_app_state::capi_app_investor_state_from_acc, capi_asset_id::CapiAssetAmount,
            common_test::lock_unlock::test_shares_locked,
            create::setup_flow::test_flow::setup_capi_asset_flow,
        },
        state::account_state::{asset_holdings, find_asset_holding_or_err},
        testing::{
            create_and_submit_txs::{
                optin_to_asset_submit, optin_to_capi_app_submit, transfer_tokens_submit,
            },
            flow::{
                lock_capi_asset_flow::lock_capi_asset_flow,
                unlock_capi_asset_flow::unlock_capi_asset_flow,
            },
            network_test_util::{create_and_distribute_funds_asset, test_init},
            test_data::{creator, investor1},
        },
    };
    use anyhow::Result;
    use mbase::dependencies::algod_for_tests;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_unlock() -> Result<()> {
        test_init()?;

        // deps

        let algod = algod_for_tests();
        let api = LocalTealApi {};
        let creator = creator();
        let investor = investor1();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;
        let capi_supply = CapiAssetAmount::new(1_000_000_000);
        let capi_deps =
            setup_capi_asset_flow(&algod, &api, &creator, capi_supply, funds_asset_id).await?;

        // preconditions

        let investor_assets_amount = CapiAssetAmount::new(1_000);

        let params = algod.suggested_transaction_params().await?;
        optin_to_asset_submit(&algod, &investor, capi_deps.asset_id.0).await?;
        optin_to_capi_app_submit(&algod, &params, &investor, capi_deps.app_id).await?;
        transfer_tokens_submit(
            &algod,
            &params,
            &creator,
            &investor.address(),
            capi_deps.asset_id.0,
            investor_assets_amount.0,
        )
        .await?;

        // flow

        lock_capi_asset_flow(
            &algod,
            &investor,
            investor_assets_amount,
            capi_deps.asset_id,
            capi_deps.app_id,
        )
        .await?;

        // double check that state is ok after locking
        test_shares_locked(
            &algod,
            &investor.address(),
            capi_deps.asset_id,
            capi_deps.app_id,
            investor_assets_amount,
            CapiAssetAmount::new(0), // the investor locked everything
            &capi_deps.app_address(),
        )
        .await?;

        // unlock the tokens we just locked
        unlock_capi_asset_flow(&algod, &investor, capi_deps.app_id, capi_deps.asset_id).await?;

        // tests

        // investor got back the locked assets

        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let asset_holding = find_asset_holding_or_err(&investor_assets, capi_deps.asset_id.0)?;
        assert_eq!(
            investor_assets_amount,
            CapiAssetAmount::new(asset_holding.amount)
        );

        // escrow lost the assets

        let app_escrow_infos = algod.account_information(&capi_deps.app_address()).await?;
        let app_escrow_assets = app_escrow_infos.assets;
        assert_eq!(2, app_escrow_assets.len()); // opted in to shares and capi token
        let capi_asset_holdings =
            asset_holdings(&algod, &capi_deps.app_address(), capi_deps.asset_id.0).await?;
        assert_eq!(0, capi_asset_holdings.0);

        // retrieving local state fails, because the investor is opted out

        let local_state_res = capi_app_investor_state_from_acc(&investor_infos, capi_deps.app_id);
        assert!(local_state_res.is_err());

        Ok(())
    }
}

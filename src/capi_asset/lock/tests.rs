#[cfg(test)]
mod tests {
    use crate::{
        api::api::LocalApi,
        capi_asset::{
            capi_asset_id::CapiAssetAmount, common_test::lock_unlock::test_shares_locked,
            create::test_flow::test_flow::setup_capi_asset_flow,
        },
        dependencies,
        testing::{
            create_and_submit_txs::{
                optin_to_asset_submit, optin_to_capi_app_submit, transfer_tokens_and_pay_fee_submit,
            },
            flow::lock_capi_asset_flow::lock_capi_asset_flow,
            network_test_util::{create_and_distribute_funds_asset, test_init},
            test_data::{creator, investor1},
        },
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    // TODO test lock when there are funds (state initialized to entitled amount, like in shares lock test)

    #[test]
    #[serial]
    async fn test_lock() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let api = LocalApi {};
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
        transfer_tokens_and_pay_fee_submit(
            &algod,
            &params,
            &creator,
            &creator,
            &investor.address(),
            capi_deps.asset_id.0,
            investor_assets_amount.val(),
        )
        .await?;

        // flow

        lock_capi_asset_flow(
            &algod,
            &investor,
            investor_assets_amount,
            capi_deps.asset_id,
            capi_deps.app_id,
            &capi_deps.escrow.address(),
        )
        .await?;

        // tests

        test_shares_locked(
            &algod,
            &investor.address(),
            capi_deps.asset_id,
            capi_deps.app_id,
            investor_assets_amount,
            CapiAssetAmount::new(0), // the investor locked everything
            &capi_deps.escrow.address(),
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_partial_lock() -> Result<()> {
        test_init()?;

        let algod = dependencies::algod_for_tests();
        let api = LocalApi {};
        let creator = creator();
        let investor = investor1();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;
        let capi_supply = CapiAssetAmount::new(1_000_000_000);
        let capi_deps =
            setup_capi_asset_flow(&algod, &api, &creator, capi_supply, funds_asset_id).await?;

        // preconditions

        let partial_lock_amount = CapiAssetAmount::new(400);
        let investor_assets_amount = CapiAssetAmount::new(partial_lock_amount.val() + 600);

        let params = algod.suggested_transaction_params().await?;
        optin_to_asset_submit(&algod, &investor, capi_deps.asset_id.0).await?;
        optin_to_capi_app_submit(&algod, &params, &investor, capi_deps.app_id).await?;
        transfer_tokens_and_pay_fee_submit(
            &algod,
            &params,
            &creator,
            &creator,
            &investor.address(),
            capi_deps.asset_id.0,
            investor_assets_amount.val(),
        )
        .await?;

        // flow

        lock_capi_asset_flow(
            &algod,
            &investor,
            partial_lock_amount,
            capi_deps.asset_id,
            capi_deps.app_id,
            &capi_deps.escrow.address(),
        )
        .await?;

        // tests

        test_shares_locked(
            &algod,
            &investor.address(),
            capi_deps.asset_id,
            capi_deps.app_id,
            partial_lock_amount,
            CapiAssetAmount::new(investor_assets_amount.val() - partial_lock_amount.val()),
            &capi_deps.escrow.address(),
        )
        .await?;

        Ok(())
    }
}

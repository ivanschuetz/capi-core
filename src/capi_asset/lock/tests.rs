#[cfg(test)]
mod tests {
    use crate::{
        capi_asset::{
            capi_asset_id::CapiAssetAmount, common_test::lock_unlock::test_shares_locked,
            create::test_flow::test_flow::setup_capi_asset_flow,
        },
        dependencies,
        testing::{
            create_and_submit_txs::{
                optin_to_app_submit, optin_to_asset_submit, transfer_tokens_and_pay_fee_submit,
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
        let creator = creator();
        let investor = investor1();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        let capi_supply = CapiAssetAmount(1_000_000_000);

        // preconditions

        let setup_res =
            setup_capi_asset_flow(&algod, &creator, capi_supply, funds_asset_id).await?;

        let investor_assets_amount = CapiAssetAmount(1_000);

        let params = algod.suggested_transaction_params().await?;
        optin_to_asset_submit(&algod, &investor, setup_res.asset_id.0).await?;
        optin_to_app_submit(&algod, &params, &investor, setup_res.app_id.0).await?;
        transfer_tokens_and_pay_fee_submit(
            &algod,
            &params,
            &creator,
            &creator,
            &investor.address(),
            setup_res.asset_id.0,
            investor_assets_amount.0,
        )
        .await?;

        // flow

        lock_capi_asset_flow(
            &algod,
            &investor,
            investor_assets_amount,
            setup_res.asset_id,
            setup_res.app_id,
            &setup_res.escrow,
        )
        .await?;

        // tests

        test_shares_locked(
            &algod,
            &investor.address(),
            setup_res.asset_id,
            setup_res.app_id,
            investor_assets_amount,
            CapiAssetAmount(0), // the investor locked everything
            setup_res.escrow.address(),
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_partial_lock() -> Result<()> {
        test_init()?;

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        let capi_supply = CapiAssetAmount(1_000_000_000);

        // preconditions

        let setup_res =
            setup_capi_asset_flow(&algod, &creator, capi_supply, funds_asset_id).await?;

        let partial_lock_amount = CapiAssetAmount(400);
        let investor_assets_amount = CapiAssetAmount(partial_lock_amount.0 + 600);

        let params = algod.suggested_transaction_params().await?;
        optin_to_asset_submit(&algod, &investor, setup_res.asset_id.0).await?;
        optin_to_app_submit(&algod, &params, &investor, setup_res.app_id.0).await?;
        transfer_tokens_and_pay_fee_submit(
            &algod,
            &params,
            &creator,
            &creator,
            &investor.address(),
            setup_res.asset_id.0,
            investor_assets_amount.0,
        )
        .await?;

        // flow

        lock_capi_asset_flow(
            &algod,
            &investor,
            partial_lock_amount,
            setup_res.asset_id,
            setup_res.app_id,
            &setup_res.escrow,
        )
        .await?;

        // tests

        test_shares_locked(
            &algod,
            &investor.address(),
            setup_res.asset_id,
            setup_res.app_id,
            partial_lock_amount,
            CapiAssetAmount(investor_assets_amount.0 - partial_lock_amount.0),
            setup_res.escrow.address(),
        )
        .await?;

        Ok(())
    }
}

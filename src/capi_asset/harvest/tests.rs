#[cfg(test)]
mod tests {
    use crate::{
        capi_asset::{
            capi_app_state::{capi_app_global_state, capi_app_investor_state},
            capi_asset_id::CapiAssetAmount,
        },
        dependencies,
        funds::FundsAmount,
        state::account_state::funds_holdings,
        testing::{
            flow::harvest_capi_flow::{harvest_capi_flow, harvest_capi_precs},
            network_test_util::{create_and_distribute_funds_asset, test_init},
            test_data::{creator, investor1},
        },
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_harvest() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        let capi_supply = CapiAssetAmount(1_000_000_000);

        let investor_capi_amount = CapiAssetAmount(100_000); // 0.0001 -> 0.01 %

        let initial_capi_funds_amount = FundsAmount(200_000);

        let harvest_amount = FundsAmount(2); // random amount < entitled harvest

        // preconditions

        let setup_res = harvest_capi_precs(
            &algod,
            &creator,
            capi_supply,
            funds_asset_id,
            &investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let investor_funds_before_harvesting =
            funds_holdings(&algod, &investor.address(), funds_asset_id).await?;

        let capi_app_total_received_before_harvesting =
            capi_app_global_state(&algod, setup_res.app_id)
                .await?
                .received;

        harvest_capi_flow(
            &algod,
            &investor,
            harvest_amount,
            funds_asset_id,
            setup_res.app_id,
            &setup_res.escrow,
        )
        .await?;

        // test

        // Investor received the harvested funds
        let harvest_funds_amount =
            funds_holdings(&algod, &investor.address(), funds_asset_id).await?;
        assert_eq!(
            investor_funds_before_harvesting.0 + harvest_amount.0,
            harvest_funds_amount.0
        );

        // Capi lost the harvested funds
        let capi_escrow_funds_amount =
            funds_holdings(&algod, &setup_res.escrow.address(), funds_asset_id).await?;
        assert_eq!(
            initial_capi_funds_amount.0 - harvest_amount.0,
            capi_escrow_funds_amount.0
        );

        // Capi app global state: test that the total received global variable didn't change (unaffected by harvesting)
        let capi_app_global_state = capi_app_global_state(&algod, setup_res.app_id).await?;
        assert_eq!(
            capi_app_total_received_before_harvesting,
            capi_app_global_state.received
        );

        // Investor local state: test that it was incremented by amount harvested
        let investor_local_state =
            capi_app_investor_state(&algod, &investor.address(), setup_res.app_id).await?;
        // harvested local state is just what they just harvested (there wasn't anything on the escrow when the investor invested)
        assert_eq!(harvest_amount.0, investor_local_state.harvested.0);
        // sanity check: the shares local state is set to the locked shares
        assert_eq!(investor_capi_amount.0, investor_local_state.shares.0);

        Ok(())
    }

    // TODO rest of tests from DAO harvest
}

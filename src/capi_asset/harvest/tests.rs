#[cfg(test)]
mod tests {
    use crate::{
        capi_asset::{
            capi_app_state::{
                capi_app_global_state, capi_app_investor_state, CapiAppGlobalState,
                CapiAppHolderState,
            },
            capi_asset_id::CapiAssetAmount,
            create::test_flow::test_flow::setup_capi_asset_flow,
            harvest::harvest::max_can_harvest_amount,
        },
        dependencies,
        funds::FundsAmount,
        state::account_state::funds_holdings,
        testing::{
            flow::harvest_capi_flow::{harvest_capi_flow, harvest_capi_precs},
            network_test_util::{
                create_and_distribute_funds_asset, test_dao_init, test_dao_init_with_deps,
                test_init, OnChainDeps, TestDeps,
            },
            test_data::capi_owner,
        },
    };
    use algonaut::core::Address;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_harvest_max_with_repeated_fractional_shares_percentage() -> Result<()> {
        test_init()?;

        let algod = dependencies::algod_for_tests();
        let capi_owner = capi_owner();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;
        let capi_flow_res = setup_capi_asset_flow(
            &algod,
            &capi_owner,
            CapiAssetAmount::new(300),
            funds_asset_id,
        )
        .await?;
        let chain_deps = OnChainDeps {
            funds_asset_id,
            capi_flow_res,
        };

        let td = &test_dao_init_with_deps(algod, &chain_deps).await?;
        let algod = &td.algod;

        let investor = &td.investor1;

        // Capi tokens owned by investor, to be able to harvest
        let investor_capi_amount = CapiAssetAmount::new(10);

        let initial_capi_funds_amount = FundsAmount::new(10_000_000);

        // 10 shares, 300 supply, percentage: 0.0333333333

        // preconditions

        let precs = harvest_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let investor_funds_before_harvesting =
            funds_holdings(algod, &investor.address(), td.funds_asset_id).await?;

        let capi_app_total_received_before_harvesting =
            capi_app_global_state(algod, td.capi_app_id).await?.received;

        let harvest_amount = max_can_harvest_amount(
            // the calculated capi fee is what's on the capi app (total received state) now
            precs.drain_res.drained_amounts.capi,
            FundsAmount::new(0),
            investor_capi_amount,
            td.capi_supply,
            td.precision,
        )?;

        harvest_capi_flow(
            algod,
            investor,
            harvest_amount,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await?;

        // test

        test_harvest_result(
            &td,
            &investor.address(),
            investor_capi_amount,
            harvest_amount,
            CapiAppGlobalState {
                // Total received didn't change (unaffected by harvesting)
                received: capi_app_total_received_before_harvesting,
            },
            // Investor received the harvested funds
            FundsAmount::new(investor_funds_before_harvesting.val() + harvest_amount.val()),
            // Capi lost the harvested funds
            FundsAmount::new(initial_capi_funds_amount.val() - harvest_amount.val()),
            &CapiAppHolderState {
                // harvested local state is what they just harvested (there wasn't anything on the escrow when the investor invested)
                harvested: harvest_amount,
                // sanity check: the shares local state is still set to the locked shares
                shares: investor_capi_amount,
            },
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_harvest_max_with_repeated_fractional_shares_percentage_plus_1_fails() -> Result<()>
    {
        test_init()?;

        let algod = dependencies::algod_for_tests();
        let capi_owner = capi_owner();

        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;
        let capi_flow_res = setup_capi_asset_flow(
            &algod,
            &capi_owner,
            CapiAssetAmount::new(300),
            funds_asset_id,
        )
        .await?;
        let chain_deps = OnChainDeps {
            funds_asset_id,
            capi_flow_res,
        };

        let td = &test_dao_init_with_deps(algod, &chain_deps).await?;
        let algod = &td.algod;

        let investor = &td.investor1;

        // Capi tokens owned by investor, to be able to harvest
        let investor_capi_amount = CapiAssetAmount::new(10);

        let initial_capi_funds_amount = FundsAmount::new(10_000_000);

        // 10 shares, 300 supply, percentage: 0.0333333333

        // preconditions

        let precs = harvest_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let harvest_amount = max_can_harvest_amount(
            // the calculated capi fee is what's on the capi app (total received state) now
            precs.drain_res.drained_amounts.capi,
            FundsAmount::new(0),
            investor_capi_amount,
            td.capi_supply,
            td.precision,
        )?;

        // The max harvest calculation and TEAL use floor to round the decimal. TEAL will reject + 1
        let res = harvest_capi_flow(
            algod,
            investor,
            harvest_amount + 1,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await;

        // test

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_capi_max_harvest() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        // Capi tokens owned by investor, to be able to harvest
        let investor_capi_amount = CapiAssetAmount::new(100_000); // 0.0001 -> 0.01 %

        let initial_capi_funds_amount = FundsAmount::new(200_000);

        // preconditions

        let precs = harvest_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let investor_funds_before_harvesting =
            funds_holdings(algod, &investor.address(), td.funds_asset_id).await?;

        let capi_app_total_received_before_harvesting =
            capi_app_global_state(algod, td.capi_app_id).await?.received;

        let harvest_amount = max_can_harvest_amount(
            // the calculated capi fee is what's on the capi app (total received state) now
            precs.drain_res.drained_amounts.capi,
            FundsAmount::new(0),
            investor_capi_amount,
            td.capi_supply,
            td.precision,
        )?;

        harvest_capi_flow(
            algod,
            investor,
            harvest_amount,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await?;

        // test

        test_harvest_result(
            &td,
            &investor.address(),
            investor_capi_amount,
            harvest_amount,
            CapiAppGlobalState {
                // Total received didn't change (unaffected by harvesting)
                received: capi_app_total_received_before_harvesting,
            },
            // Investor received the harvested funds
            FundsAmount::new(investor_funds_before_harvesting.val() + harvest_amount.val()),
            // Capi lost the harvested funds
            FundsAmount::new(initial_capi_funds_amount.val() - harvest_amount.val()),
            &CapiAppHolderState {
                // harvested local state is what they just harvested (there wasn't anything on the escrow when the investor invested)
                harvested: harvest_amount,
                // sanity check: the shares local state is still set to the locked shares
                shares: investor_capi_amount,
            },
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_capi_harvest_more_than_max() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        // Capi tokens owned by investor, to be able to harvest
        let investor_capi_amount = CapiAssetAmount::new(100_000); // 0.0001 -> 0.01 %

        let initial_capi_funds_amount = FundsAmount::new(200_000);

        // preconditions

        let precs = harvest_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let harvest_amount = max_can_harvest_amount(
            // the calculated capi fee is what's on the capi app (total received state) now
            precs.drain_res.drained_amounts.capi,
            FundsAmount::new(0),
            investor_capi_amount,
            td.capi_supply,
            td.precision,
        )?;

        let res = harvest_capi_flow(
            algod,
            investor,
            // we harvest 1 asset more than max allowed
            harvest_amount + 1,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await;
        log::debug!("res: {:?}", res);

        // test

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_2_successful_harvests() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        // Capi tokens owned by investor, to be able to harvest
        let investor_capi_amount = CapiAssetAmount::new(100_000); // 0.0001 -> 0.01 %

        let initial_capi_funds_amount = FundsAmount::new(200_000);

        // preconditions

        harvest_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let investor_funds_before_harvesting =
            funds_holdings(algod, &investor.address(), td.funds_asset_id).await?;

        let capi_app_total_received_before_harvesting =
            capi_app_global_state(algod, td.capi_app_id).await?.received;

        let harvest_amount = FundsAmount::new(5); // just an amount low enough so we can harvest 2x

        harvest_capi_flow(
            algod,
            investor,
            harvest_amount,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await?;

        harvest_capi_flow(
            algod,
            investor,
            harvest_amount,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await?;

        // test

        let total_harvest_amount = harvest_amount * 2;

        test_harvest_result(
            &td,
            &investor.address(),
            investor_capi_amount,
            total_harvest_amount,
            CapiAppGlobalState {
                // Total received didn't change (unaffected by harvesting)
                received: capi_app_total_received_before_harvesting,
            },
            // Investor received the harvested funds
            investor_funds_before_harvesting + total_harvest_amount,
            // Capi lost the harvested funds
            FundsAmount::new(initial_capi_funds_amount.val() - total_harvest_amount.val()),
            &CapiAppHolderState {
                // harvested local state is what they just harvested (there wasn't anything on the escrow when the investor invested)
                harvested: total_harvest_amount,
                // sanity check: the shares local state is still set to the locked shares
                shares: investor_capi_amount,
            },
        )
        .await?;

        Ok(())
    }

    async fn test_harvest_result(
        td: &TestDeps,
        investor: &Address,
        investor_capi_amount: CapiAssetAmount,
        harvest_amount: FundsAmount,

        expected_global_state: CapiAppGlobalState,
        expected_investor_funds: FundsAmount,
        expected_capi_escrow_funds: FundsAmount,
        expected_investor_local_state: &CapiAppHolderState,
    ) -> Result<()> {
        let algod = &td.algod;

        let harvest_funds_amount = funds_holdings(algod, &investor, td.funds_asset_id).await?;
        assert_eq!(expected_investor_funds, harvest_funds_amount);

        let capi_escrow_funds_amount =
            funds_holdings(algod, td.capi_escrow.address(), td.funds_asset_id).await?;
        assert_eq!(expected_capi_escrow_funds, capi_escrow_funds_amount);

        let capi_app_global_state = capi_app_global_state(&algod, td.capi_app_id).await?;
        assert_eq!(expected_global_state, capi_app_global_state);

        let investor_local_state =
            capi_app_investor_state(algod, &investor, td.capi_app_id).await?;
        assert_eq!(expected_investor_local_state, &investor_local_state);
        assert_eq!(harvest_amount.0, investor_local_state.harvested.0);
        assert_eq!(investor_capi_amount.0, investor_local_state.shares.0);

        Ok(())
    }
}

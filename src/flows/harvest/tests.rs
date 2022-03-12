#[cfg(test)]
mod tests {
    use crate::{
        flows::{
            create_project::{
                create_project_specs::CreateProjectSpecs, model::CreateSharesSpecs,
                share_amount::ShareAmount,
            },
            harvest::harvest::max_can_harvest_amount,
        },
        funds::{FundsAmount, FundsAssetId},
        state::{
            account_state::funds_holdings,
            central_app_state::{central_global_state, central_investor_state_from_acc},
        },
        testing::{
            flow::harvest_flow::{harvest_flow, harvest_precs},
            network_test_util::test_dao_init,
            TESTS_DEFAULT_PRECISION,
        },
    };
    use algonaut::{algod::v2::Algod, core::Address, transaction::account::Account};
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_harvest() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let harvester = &td.investor2;

        // flow

        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let precision = TESTS_DEFAULT_PRECISION;

        let precs = harvest_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            drainer,
            harvester,
        )
        .await?;

        let harvest_amount = max_can_harvest_amount(
            precs.drain_res.drained_amounts.dao,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            precision,
            td.specs.investors_part(),
        )?;

        let res = harvest_flow(&td, &precs.project, harvester, harvest_amount).await?;

        // test

        test_harvest_result(
            &algod,
            &harvester,
            res.project.central_app_id,
            td.funds_asset_id,
            res.project.central_escrow.address(),
            res.project.customer_escrow.address(),
            precs.drain_res.drained_amounts.dao,
            // harvester got the amount
            res.harvester_balance_before_harvesting + res.harvest,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.harvest,
            // double check shares local state
            buy_share_amount,
            // only one harvest: local state is the harvested amount
            res.harvest,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_harvest_more_than_max() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let harvester = &td.investor2;

        // precs

        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let precision = TESTS_DEFAULT_PRECISION;

        let precs = harvest_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            drainer,
            harvester,
        )
        .await?;

        let central_state = central_global_state(&algod, precs.project.central_app_id).await?;
        let harvest_amount = max_can_harvest_amount(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            precision,
            td.specs.investors_part(),
        )?;
        log::debug!("Harvest amount: {}", harvest_amount);

        // flow

        // we harvest 1 microalgo (smallest possible increment) more than max allowed
        let res = harvest_flow(&td, &precs.project, &harvester, harvest_amount + 1).await;
        log::debug!("res: {:?}", res);

        // test

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_harvest_max_with_repeated_fractional_shares_percentage() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let harvester = &td.investor2;

        // precs

        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let precision = TESTS_DEFAULT_PRECISION;
        let specs = CreateProjectSpecs::new(
            "Pancakes ltd".to_owned(),
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat".to_owned(),
            CreateSharesSpecs {
                token_name: "PCK".to_owned(),
                supply: ShareAmount::new(300),
            },
            ShareAmount::new(120),
            FundsAmount::new(5_000_000),
            "https://placekitten.com/200/300".to_owned(),
            "https://twitter.com/capi_fin".to_owned(),
        )?;
        // 10 shares, 300 supply, 100% investor's share, percentage: 0.0333333333

        let precs = harvest_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            &drainer,
            &harvester,
        )
        .await?;

        let central_state = central_global_state(&algod, precs.project.central_app_id).await?;
        log::debug!("central_total_received: {:?}", central_state.received);

        let harvest_amount = max_can_harvest_amount(
            central_state.received,
            FundsAmount::new(0),
            specs.shares.supply,
            buy_share_amount,
            precision,
            specs.investors_part(),
        )?;
        log::debug!("Harvest amount: {}", harvest_amount);

        // flow

        let res = harvest_flow(&td, &precs.project, &harvester, harvest_amount).await?;

        // test

        test_harvest_result(
            &algod,
            &harvester,
            res.project.central_app_id,
            td.funds_asset_id,
            res.project.central_escrow.address(),
            res.project.customer_escrow.address(),
            precs.drain_res.drained_amounts.dao,
            // harvester got the amount
            res.harvester_balance_before_harvesting + res.harvest,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.harvest,
            // double check shares local state
            buy_share_amount,
            // only one harvest: local state is the harvested amount
            res.harvest,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_2_successful_harvests() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let harvester = &td.investor2;

        // flow

        let buy_share_amount = ShareAmount::new(20);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let harvest_amount = FundsAmount::new(200_000); // just an amount low enough so we can harvest 2x

        let precs = harvest_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            &drainer,
            &harvester,
        )
        .await?;
        let res1 = harvest_flow(&td, &precs.project, &harvester, harvest_amount).await?;
        let res2 = harvest_flow(&td, &precs.project, &harvester, harvest_amount).await?;

        // test

        let total_expected_harvested_amount = res1.harvest + res2.harvest;
        test_harvest_result(
            &algod,
            &harvester,
            res2.project.central_app_id,
            td.funds_asset_id,
            res2.project.central_escrow.address(),
            res2.project.customer_escrow.address(),
            precs.drain_res.drained_amounts.dao,
            // 2 harvests: local state is the total harvested amount
            res1.harvester_balance_before_harvesting + total_expected_harvested_amount,
            // central lost the amount
            precs.central_escrow_balance_after_drain - total_expected_harvested_amount,
            // double check shares local state
            buy_share_amount,
            // 2 harvests: local state is the total harvested amount
            total_expected_harvested_amount,
        )
        .await?;

        Ok(())
    }

    // TODO like test_2_successful_harvests but not enough funds for 2nd harvest
    // (was accidentally partly tested with test_2_successful_harvests, as the default accounts didn't have enough funds for the 2nd harvest,
    // but should be a permanent test of course)

    async fn test_harvest_result(
        algod: &Algod,
        harvester: &Account,
        central_app_id: u64,
        funds_asset_id: FundsAssetId,
        central_escrow_address: &Address,
        customer_escrow_address: &Address,
        // this parameter isn't ideal: it assumes that we did a (one) drain before harvesting
        // for now letting it there as it's a quick refactoring
        // arguably needed, it tests basically that the total received global state isn't affected by harvests
        // (otherwise this is/should be already tested in the drain logic)
        drained_amount: FundsAmount,
        expected_harvester_balance: FundsAmount,
        expected_central_balance: FundsAmount,
        expected_shares: ShareAmount,
        expected_harvested_total: FundsAmount,
    ) -> Result<()> {
        let harvest_funds_amount =
            funds_holdings(algod, &harvester.address(), funds_asset_id).await?;
        let central_escrow_funds_amount =
            funds_holdings(algod, central_escrow_address, funds_asset_id).await?;

        assert_eq!(expected_harvester_balance, harvest_funds_amount);
        assert_eq!(expected_central_balance, central_escrow_funds_amount);

        // the total received didn't change
        // (i.e. same as expected after draining, harvesting doesn't affect it)
        let global_state = central_global_state(algod, central_app_id).await?;
        assert_eq!(global_state.received, drained_amount);

        // sanity check: global state addresses are set
        assert_eq!(&global_state.central_escrow, central_escrow_address);
        assert_eq!(&global_state.customer_escrow, customer_escrow_address);

        // harvester local state: test that it was incremented by amount harvested
        // Only one local variable used
        let harvester_account = algod.account_information(&harvester.address()).await?;
        assert_eq!(1, harvester_account.apps_local_state.len());

        // check local state

        let investor_state = central_investor_state_from_acc(&harvester_account, central_app_id)?;

        // double-check shares count (not directly related to this test)
        assert_eq!(expected_shares, investor_state.shares);
        // check harvested total local state
        assert_eq!(expected_harvested_total, investor_state.harvested);

        Ok(())
    }
}

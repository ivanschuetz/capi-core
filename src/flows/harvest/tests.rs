#[cfg(test)]
mod tests {
    use algonaut::{
        algod::v2::Algod,
        core::{Address, MicroAlgos},
        transaction::account::Account,
    };
    use anyhow::Result;
    use data_encoding::BASE64;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::{
            create_project::model::{CreateProjectSpecs, CreateSharesSpecs},
            harvest::harvest::{investor_can_harvest_amount_calc, FIXED_FEE},
        },
        state::central_app_state::{central_global_state, central_investor_state_from_acc},
        testing::{
            flow::harvest_flow::{harvest_flow, harvest_precs},
            network_test_util::test_init,
            project_general::check_schema,
            test_data::{creator, customer, investor1, investor2, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };

    #[test]
    #[serial]
    async fn test_harvest() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let harvester = investor2();
        let customer = customer();

        let specs = project_specs();

        // flow

        let buy_asset_amount = 10;
        let central_funds = MicroAlgos(10 * 1_000_000);
        let harvest_amount = MicroAlgos(400_000); // calculated manually
        let precision = TESTS_DEFAULT_PRECISION;

        let precs = harvest_precs(
            &algod,
            &creator,
            &specs,
            &harvester,
            &drainer,
            &customer,
            buy_asset_amount,
            central_funds,
            precision,
        )
        .await?;
        let res = harvest_flow(&algod, &precs.project, &harvester, harvest_amount).await?;

        // test

        test_harvest_result(
            &algod,
            &harvester,
            res.project.central_app_id,
            res.project.central_escrow.address(),
            precs.drain_res.drained_amount,
            // harvester got the amount - app call fee - pay for escrow fee - fee to pay for escrow fee
            res.harvester_balance_before_harvesting + res.harvest - FIXED_FEE * 3,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.harvest,
            // double check shares local state
            buy_asset_amount,
            // only one harvest: local state is the harvested amount
            res.harvest.0,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_harvest_max() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let harvester = investor2();
        let customer = customer();

        let specs = project_specs();

        // precs

        let buy_asset_amount = 10;
        let central_funds = MicroAlgos(10 * 1_000_000);
        let precision = TESTS_DEFAULT_PRECISION;

        let precs = harvest_precs(
            &algod,
            &creator,
            &specs,
            &harvester,
            &drainer,
            &customer,
            buy_asset_amount,
            central_funds,
            precision,
        )
        .await?;

        let central_state = central_global_state(&algod, precs.project.central_app_id).await?;

        let harvest_amount = investor_can_harvest_amount_calc(
            central_state.received,
            MicroAlgos(0),
            buy_asset_amount,
            specs.shares.count,
            precision,
            specs.investors_share,
        );
        log::debug!("Harvest amount: {}", harvest_amount);

        // flow

        let res = harvest_flow(&algod, &precs.project, &harvester, harvest_amount).await?;

        // test

        test_harvest_result(
            &algod,
            &harvester,
            res.project.central_app_id,
            res.project.central_escrow.address(),
            precs.drain_res.drained_amount,
            // harvester got the amount - app call fee - pay for escrow fee - fee to pay for escrow fee
            res.harvester_balance_before_harvesting + res.harvest - FIXED_FEE * 3,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.harvest,
            // double check shares local state
            buy_asset_amount,
            // only one harvest: local state is the harvested amount
            res.harvest.0,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_harvest_more_than_max() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let harvester = investor2();
        let customer = customer();

        let specs = project_specs();

        // precs

        let buy_asset_amount = 10;
        let central_funds = MicroAlgos(10 * 1_000_000);
        let precision = TESTS_DEFAULT_PRECISION;

        let precs = harvest_precs(
            &algod,
            &creator,
            &specs,
            &harvester,
            &drainer,
            &customer,
            buy_asset_amount,
            central_funds,
            precision,
        )
        .await?;

        let central_state = central_global_state(&algod, precs.project.central_app_id).await?;
        let harvest_amount = investor_can_harvest_amount_calc(
            central_state.received,
            MicroAlgos(0),
            buy_asset_amount,
            specs.shares.count,
            precision,
            specs.investors_share,
        );
        log::debug!("Harvest amount: {}", harvest_amount);

        // flow

        // we harvest 1 microalgo (smallest possible increment) more than max allowed
        let res = harvest_flow(&algod, &precs.project, &harvester, harvest_amount + 1).await;
        log::debug!("res: {:?}", res);

        // test

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_harvest_max_with_repeated_fractional_shares_percentage() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let harvester = investor2();
        let customer = customer();

        // precs

        let buy_asset_amount = 10;
        let central_funds = MicroAlgos(10 * 1_000_000);
        let precision = TESTS_DEFAULT_PRECISION;
        let specs = CreateProjectSpecs {
            name: "Pancakes ltd".to_owned(),
            shares: CreateSharesSpecs {
                token_name: "PCK".to_owned(),
                count: 300,
            },
            asset_price: MicroAlgos(5_000_000),
            investors_share: 100,
        };
        // 10 shares, 300 supply, 100% investor's share, percentage: 0.0333333333

        let precs = harvest_precs(
            &algod,
            &creator,
            &specs,
            &harvester,
            &drainer,
            &customer,
            buy_asset_amount,
            central_funds,
            precision,
        )
        .await?;

        let central_state = central_global_state(&algod, precs.project.central_app_id).await?;
        log::debug!("central_total_received: {}", central_state.received);

        let harvest_amount = investor_can_harvest_amount_calc(
            central_state.received,
            MicroAlgos(0),
            buy_asset_amount,
            specs.shares.count,
            precision,
            specs.investors_share,
        );
        log::debug!("Harvest amount: {}", harvest_amount);

        // flow

        let res = harvest_flow(&algod, &precs.project, &harvester, harvest_amount).await?;

        // test

        test_harvest_result(
            &algod,
            &harvester,
            res.project.central_app_id,
            res.project.central_escrow.address(),
            precs.drain_res.drained_amount,
            // harvester got the amount - app call fee - pay for escrow fee - fee to pay for escrow fee
            res.harvester_balance_before_harvesting + res.harvest - FIXED_FEE * 3,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.harvest,
            // double check shares local state
            buy_asset_amount,
            // only one harvest: local state is the harvested amount
            res.harvest.0,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_2_successful_harvests() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let harvester = investor2();
        let customer = customer();

        // flow

        let buy_asset_amount = 20;
        let central_funds = MicroAlgos(10 * 1_000_000);
        let harvest_amount = MicroAlgos(200_000); // just an amount low enough so we can harvest 2x
        let precision = TESTS_DEFAULT_PRECISION;

        let precs = harvest_precs(
            &algod,
            &creator,
            &project_specs(),
            &harvester,
            &drainer,
            &customer,
            // 20 with 100 supply (TODO pass supply or just create specs here) means that we're entitled to 20% of total drained
            // so 20% of 10 algos (TODO pass draining amount to harvest_precs), which is 2 Algos
            // we harvest 1 Algo 2x -> success
            buy_asset_amount,
            central_funds,
            precision,
        )
        .await?;
        let res1 = harvest_flow(&algod, &precs.project, &harvester, harvest_amount).await?;
        let res2 = harvest_flow(&algod, &precs.project, &harvester, harvest_amount).await?;

        // test

        let total_expected_harvested_amount = res1.harvest.0 + res2.harvest.0;
        test_harvest_result(
            &algod,
            &harvester,
            res2.project.central_app_id,
            res2.project.central_escrow.address(),
            precs.drain_res.drained_amount,
            // 2 harvests: local state is the total harvested amount
            // FEES:
            // one harvest -> 3x: app call fee, pay for escrow fee, fee to pay for escrow fee,
            // two harvests -> 3 * 2
            res1.harvester_balance_before_harvesting + total_expected_harvested_amount
                - FIXED_FEE * (3 * 2),
            // central lost the amount
            precs.central_escrow_balance_after_drain - total_expected_harvested_amount,
            // double check shares local state
            buy_asset_amount,
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
        central_escrow_address: &Address,
        // this parameter isn't ideal: it assumes that we did a (one) drain before harvesting
        // for now letting it there as it's a quick refactoring
        // arguably needed, it tests basically that the total received global state isn't affected by harvests
        // (otherwise this is/should be already tested in the drain logic)
        drained_amount: MicroAlgos,
        expected_harvester_balance: MicroAlgos,
        expected_central_balance: MicroAlgos,
        expected_shares: u64,
        expected_harvested_total: u64,
    ) -> Result<()> {
        let harvester_account = algod.account_information(&harvester.address()).await?;
        let central_escrow_balance = algod
            .account_information(&central_escrow_address)
            .await?
            .amount;

        assert_eq!(expected_harvester_balance, harvester_account.amount);
        assert_eq!(expected_central_balance, central_escrow_balance);

        // Central global state: test that the total received global variable didn't change
        // (i.e. same as expected after draining, harvesting doesn't affect it)
        let app = algod.application_information(central_app_id).await?;
        assert_eq!(1, app.params.global_state.len());
        let global_key_value = &app.params.global_state[0];
        assert_eq!(BASE64.encode(b"CentralReceivedTotal"), global_key_value.key);
        assert_eq!(Vec::<u8>::new(), global_key_value.value.bytes);
        // after drain, the central received total gs is the amount that was drained
        // (note that this is not affected by harvests)
        assert_eq!(drained_amount.0, global_key_value.value.uint);
        // values not documented: 1 is byte slice and 2 uint
        // https://forum.algorand.org/t/interpreting-goal-app-read-response/2711
        assert_eq!(2, global_key_value.value.value_type);

        // harvester local state: test that it was incremented by amount harvested
        // Only one local variable used
        assert_eq!(1, harvester_account.apps_local_state.len());
        // check local state

        let investor_state = central_investor_state_from_acc(&harvester_account, central_app_id)?;

        // double-check shares count (not directly related to this test)
        assert_eq!(expected_shares, investor_state.shares);
        // check harvested total local state
        assert_eq!(
            MicroAlgos(expected_harvested_total),
            investor_state.harvested
        );

        // double check (_very_ unlikely to be needed)
        check_schema(&app);

        Ok(())
    }
}

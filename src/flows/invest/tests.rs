#[cfg(test)]
mod tests {
    use crate::flows::create_project::model::Project;
    use crate::flows::create_project::share_amount::ShareAmount;
    use crate::flows::create_project::storage::load_project::ProjectId;
    use crate::flows::harvest::harvest::calculate_entitled_harvest;
    use crate::funds::{FundsAmount, FundsAssetId};
    use crate::network_util::wait_for_pending_transaction;
    use crate::queries::my_projects::my_current_invested_projects;
    use crate::state::account_state::{
        find_asset_holding_or_err, funds_holdings, funds_holdings_from_account,
    };
    use crate::state::central_app_state::{
        central_global_state, central_investor_state, central_investor_state_from_acc,
    };
    use crate::testing::flow::create_project_flow::{create_project_flow, programs};
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::flow::invest_in_project_flow::{invests_flow, invests_optins_flow};
    use crate::testing::flow::stake_flow::stake_flow;
    use crate::testing::flow::unstake_flow::unstake_flow;
    use crate::testing::network_test_util::{create_and_distribute_funds_asset, test_init};
    use crate::testing::test_data::{customer, investor2};
    use crate::testing::TESTS_DEFAULT_PRECISION;
    use crate::{
        dependencies,
        testing::test_data::creator,
        testing::test_data::{investor1, project_specs},
    };
    use algonaut::algod::v2::Algod;
    use algonaut::transaction::account::Account;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invests_flow() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let buy_share_amount = ShareAmount(10);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // flow

        let flow_res = invests_flow(
            &algod,
            &investor,
            buy_share_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // staking escrow tests

        let staking_escrow_infos = algod
            .account_information(project.project.staking_escrow.address())
            .await?;
        // staking escrow received the shares
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len());
        assert_eq!(buy_share_amount.0, staking_escrow_assets[0].amount);
        // staking escrow doesn't send any transactions so not testing balances (we could "double check" though)

        // investor tests

        let investor_infos = algod.account_information(&investor.address()).await?;
        let central_investor_state =
            central_investor_state_from_acc(&investor_infos, project.project.central_app_id)?;

        // investor has shares
        assert_eq!(buy_share_amount, central_investor_state.shares);

        // check that the project id was initialized
        assert_eq!(project.project_id, central_investor_state.project_id);

        // check that harvested is 0 (nothing harvested yet)
        assert_eq!(FundsAmount(0), central_investor_state.harvested);

        // double check: investor didn't receive any shares

        let investor_assets = investor_infos.assets.clone();
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset =
            find_asset_holding_or_err(&investor_assets, project.project.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // investor lost algos and fees
        let investor_holdings = funds_holdings_from_account(&investor_infos, funds_asset_id)?;
        let paid_amount = specs.share_price.0 * buy_share_amount.0;
        assert_eq!(
            flow_res.investor_initial_amount - paid_amount,
            investor_holdings
        );

        // invest escrow tests

        let invest_escrow = flow_res.project.invest_escrow;
        let invest_escrow_infos = algod.account_information(invest_escrow.address()).await?;
        let invest_escrow_held_assets = invest_escrow_infos.assets;
        // investing escrow lost the bought assets
        assert_eq!(invest_escrow_held_assets.len(), 1);
        assert_eq!(
            invest_escrow_held_assets[0].asset_id,
            flow_res.project.shares_asset_id
        );
        assert_eq!(
            invest_escrow_held_assets[0].amount,
            flow_res.project.specs.shares.supply.0 - buy_share_amount.0
        );

        // central escrow tests

        // central escrow received paid algos
        let central_escrow_holdings = funds_holdings(
            &algod,
            &project.project.central_escrow.address(),
            funds_asset_id,
        )
        .await?;
        assert_eq!(
            flow_res.central_escrow_initial_amount + paid_amount,
            central_escrow_holdings
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_investing_twice() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let buy_share_amount = ShareAmount(10);
        let buy_share_amount2 = ShareAmount(20);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // flow

        invests_flow(
            &algod,
            &investor,
            buy_share_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // double check: investor has shares for first investment
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(buy_share_amount, investor_state.shares);

        invests_flow(
            &algod,
            &investor,
            buy_share_amount2,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // tests

        // investor has shares for both investments
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(
            buy_share_amount.0 + buy_share_amount2.0,
            investor_state.shares.0
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_investing_and_staking() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let stake_amount = ShareAmount(10);
        let invest_amount = ShareAmount(20);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // for user to have some free shares (assets) to stake
        buy_and_unstake_shares(
            &algod,
            &investor,
            &project.project,
            stake_amount,
            &project.project_id,
            funds_asset_id,
        )
        .await?;

        // flow

        // buy shares: automatically staked
        invests_optins_flow(&algod, &investor, &project.project).await?; // optin again: unstaking opts user out
        invests_flow(
            &algod,
            &investor,
            invest_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // double check: investor has shares for first investment
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(invest_amount, investor_state.shares);

        // stake shares
        stake_flow(
            &algod,
            &project.project,
            &project.project_id,
            &investor,
            stake_amount,
        )
        .await?;

        // tests

        // investor has shares for investment + staking
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(stake_amount.0 + invest_amount.0, investor_state.shares.0);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_staking_and_investing() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let stake_amount = ShareAmount(10);
        let invest_amount = ShareAmount(20);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // for user to have some free shares (assets) to stake
        buy_and_unstake_shares(
            &algod,
            &investor,
            &project.project,
            stake_amount,
            &project.project_id,
            funds_asset_id,
        )
        .await?;

        // flow

        // stake shares
        invests_optins_flow(&algod, &investor, &project.project).await?; // optin again: unstaking opts user out
        stake_flow(
            &algod,
            &project.project,
            &project.project_id,
            &investor,
            stake_amount,
        )
        .await?;

        // double check: investor has staked shares
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(stake_amount, investor_state.shares);

        // buy shares: automatically staked
        invests_flow(
            &algod,
            &investor,
            invest_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // tests

        // investor has shares for investment + staking
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(stake_amount.0 + invest_amount.0, investor_state.shares.0);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_staking_twice() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let stake_amount1 = ShareAmount(10);
        let stake_amount2 = ShareAmount(20);
        // an amount we unstake and will not stake again, to make the test a little more robust
        let invest_amount_not_stake = ShareAmount(5);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // for user to have free shares (assets) to stake
        buy_and_unstake_shares(
            &algod,
            &investor,
            &project.project,
            ShareAmount(stake_amount1.0 + stake_amount2.0 + invest_amount_not_stake.0),
            &project.project_id,
            funds_asset_id,
        )
        .await?;

        // flow

        // stake shares
        invests_optins_flow(&algod, &investor, &project.project).await?; // optin again: unstaking opts user out
        stake_flow(
            &algod,
            &project.project,
            &project.project_id,
            &investor,
            stake_amount1,
        )
        .await?;

        // double check: investor has staked shares
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(stake_amount1, investor_state.shares);

        // stake more shares
        stake_flow(
            &algod,
            &project.project,
            &project.project_id,
            &investor,
            stake_amount2,
        )
        .await?;

        // tests

        // investor has shares for investment + staking
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(stake_amount1.0 + stake_amount2.0, investor_state.shares.0);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invest_after_drain_inits_already_harvested_correctly() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let drainer = investor2();
        let customer = customer();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let buy_share_amount = ShareAmount(10);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        // add some funds
        let central_funds = FundsAmount(10 * 1_000_000);
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            funds_asset_id,
            central_funds,
            &project.project,
        )
        .await?;

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // flow
        invests_flow(
            &algod,
            &investor,
            buy_share_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // tests

        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        let central_state = central_global_state(&algod, project.project.central_app_id).await?;

        let investor_entitled_harvest = calculate_entitled_harvest(
            central_state.received,
            project.project.specs.shares.supply,
            buy_share_amount,
            TESTS_DEFAULT_PRECISION,
            project.project.specs.investors_part(),
        );

        // investing inits the "harvested" amount to entitled amount (to prevent double harvest)
        assert_eq!(investor_entitled_harvest, investor_state.harvested);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_stake_after_drain_inits_already_harvested_correctly() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();
        let drainer = investor2();
        let customer = customer();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let buy_share_amount = ShareAmount(10);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        // add some funds
        let central_funds = FundsAmount(10 * 1_000_000);
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            funds_asset_id,
            central_funds,
            &project.project,
        )
        .await?;

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // for user to have some free shares (assets) to stake
        buy_and_unstake_shares(
            &algod,
            &investor,
            &project.project,
            buy_share_amount,
            &project.project_id,
            funds_asset_id,
        )
        .await?;

        // flow
        invests_optins_flow(&algod, &investor, &project.project).await?; // optin again: unstaking opts user out
        stake_flow(
            &algod,
            &project.project,
            &project.project_id,
            &investor,
            buy_share_amount,
        )
        .await?;

        // tests

        let investor_state =
            central_investor_state(&algod, &investor.address(), project.project.central_app_id)
                .await?;
        let central_state = central_global_state(&algod, project.project.central_app_id).await?;

        let investor_entitled_harvest = calculate_entitled_harvest(
            central_state.received,
            project.project.specs.shares.supply,
            buy_share_amount,
            TESTS_DEFAULT_PRECISION,
            project.project.specs.investors_part(),
        );

        // staking inits the "harvested" amount to entitled amount (to prevent double harvest)
        assert_eq!(investor_entitled_harvest, investor_state.harvested);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    #[ignore] // indexer pause
    async fn test_query_my_investment() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let indexer = dependencies::indexer_for_tests();
        let creator = creator();
        let investor = investor1();
        let funds_asset_id = create_and_distribute_funds_asset(&algod).await?;

        // UI
        let buy_share_amount = ShareAmount(10);
        let specs = project_specs();

        let project = create_project_flow(
            &algod,
            &creator,
            &specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // flow

        invests_flow(
            &algod,
            &investor,
            buy_share_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // check that the invested projects query returns the project where the user invested

        // // give time for indexing
        std::thread::sleep(std::time::Duration::from_secs(10));

        let my_invested_projects = my_current_invested_projects(
            &algod,
            &indexer,
            &investor.address(),
            &programs()?.escrows,
        )
        .await?;

        assert_eq!(1, my_invested_projects.len());
        assert_eq!(project.project_id, my_invested_projects[0].id);
        assert_eq!(project.project, my_invested_projects[0].project);

        Ok(())
    }

    async fn buy_and_unstake_shares(
        algod: &Algod,
        investor: &Account,
        project: &Project,
        share_amount: ShareAmount,
        project_id: &ProjectId,
        funds_asset_id: FundsAssetId,
    ) -> Result<()> {
        invests_flow(
            &algod,
            &investor,
            share_amount,
            funds_asset_id,
            &project,
            project_id,
        )
        .await?;
        let unstake_tx_id = unstake_flow(&algod, &project, &investor, share_amount).await?;
        wait_for_pending_transaction(&algod, &unstake_tx_id).await?;
        Ok(())
    }
}

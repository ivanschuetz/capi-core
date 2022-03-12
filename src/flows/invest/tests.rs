#[cfg(test)]
mod tests {
    use crate::flows::create_project::model::Project;
    use crate::flows::create_project::share_amount::ShareAmount;
    use crate::flows::create_project::storage::load_project::ProjectId;
    use crate::flows::harvest::harvest::calculate_entitled_harvest;
    use crate::funds::FundsAmount;
    use crate::network_util::wait_for_pending_transaction;
    use crate::queries::my_projects::my_current_invested_projects;
    use crate::state::account_state::{
        find_asset_holding_or_err, funds_holdings, funds_holdings_from_account,
    };
    use crate::state::central_app_state::{
        central_global_state, central_investor_state, central_investor_state_from_acc,
    };
    use crate::testing::flow::create_project_flow::create_project_flow;
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::flow::invest_in_project_flow::{invests_flow, invests_optins_flow};
    use crate::testing::flow::lock_flow::lock_flow;
    use crate::testing::flow::unlock_flow::unlock_flow;
    use crate::testing::network_test_util::{test_dao_init, TestDeps};
    use crate::testing::test_data::investor2;
    use crate::testing::test_data::project_specs;
    use crate::testing::TESTS_DEFAULT_PRECISION;
    use algonaut::transaction::account::Account;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invests_flow() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);
        let specs = project_specs();

        let project = create_project_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &project.project).await?;

        // flow

        let flow_res = invests_flow(
            &td,
            &investor,
            buy_share_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // locking escrow tests

        let locking_escrow_infos = algod
            .account_information(project.project.locking_escrow.address())
            .await?;
        // locking escrow received the shares
        let locking_escrow_assets = locking_escrow_infos.assets;
        assert_eq!(1, locking_escrow_assets.len());
        assert_eq!(buy_share_amount.0, locking_escrow_assets[0].amount);
        // locking escrow doesn't send any transactions so not testing balances (we could "double check" though)

        // investor tests

        let investor_infos = algod.account_information(&investor.address()).await?;
        let central_investor_state =
            central_investor_state_from_acc(&investor_infos, project.project.central_app_id)?;

        // investor has shares
        assert_eq!(buy_share_amount, central_investor_state.shares);

        // check that the project id was initialized
        assert_eq!(project.project_id, central_investor_state.project_id);

        // check that harvested is 0 (nothing harvested yet)
        assert_eq!(FundsAmount::new(0), central_investor_state.harvested);

        // double check: investor didn't receive any shares

        let investor_assets = investor_infos.assets.clone();
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset =
            find_asset_holding_or_err(&investor_assets, project.project.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // investor lost algos and fees
        let investor_holdings = funds_holdings_from_account(&investor_infos, td.funds_asset_id)?;
        let paid_amount = specs.share_price.val() * buy_share_amount.val();
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
            flow_res.project.specs.shares.supply.val() - buy_share_amount.val()
        );

        // central escrow tests

        // central escrow received paid algos
        let central_escrow_holdings = funds_holdings(
            &algod,
            &project.project.central_escrow.address(),
            td.funds_asset_id,
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
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);
        let buy_share_amount2 = ShareAmount::new(20);

        let project = create_project_flow(td).await?;

        // precs

        invests_optins_flow(&algod, &investor, &project.project).await?;

        // flow

        invests_flow(
            td,
            investor,
            buy_share_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // double check: investor has shares for first investment
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(buy_share_amount, investor_state.shares);

        invests_flow(
            td,
            investor,
            buy_share_amount2,
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
            buy_share_amount.val() + buy_share_amount2.val(),
            investor_state.shares.val()
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_investing_and_locking() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let lock_amount = ShareAmount::new(10);
        let invest_amount = ShareAmount::new(20);

        let project = create_project_flow(td).await?;

        // precs

        invests_optins_flow(algod, investor, &project.project).await?;

        // for user to have some free shares (assets) to lock
        buy_and_unlock_shares(
            td,
            investor,
            &project.project,
            lock_amount,
            &project.project_id,
        )
        .await?;

        // flow

        // buy shares: automatically locked
        invests_optins_flow(algod, investor, &project.project).await?; // optin again: unlocking opts user out
        invests_flow(
            td,
            investor,
            invest_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // double check: investor has shares for first investment
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(invest_amount, investor_state.shares);

        // lock shares
        lock_flow(
            algod,
            &project.project,
            &project.project_id,
            investor,
            lock_amount,
        )
        .await?;

        // tests

        // investor has shares for investment + locking
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(
            lock_amount.val() + invest_amount.val(),
            investor_state.shares.val()
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_locking_and_investing() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let lock_amount = ShareAmount::new(10);
        let invest_amount = ShareAmount::new(20);

        let project = create_project_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &project.project).await?;

        // for user to have some free shares (assets) to lock
        buy_and_unlock_shares(
            td,
            investor,
            &project.project,
            lock_amount,
            &project.project_id,
        )
        .await?;

        // flow

        // lock shares
        invests_optins_flow(algod, investor, &project.project).await?; // optin again: unlocking opts user out
        lock_flow(
            &algod,
            &project.project,
            &project.project_id,
            &investor,
            lock_amount,
        )
        .await?;

        // double check: investor has locked shares
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(lock_amount, investor_state.shares);

        // buy shares: automatically locked
        invests_flow(
            td,
            investor,
            invest_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // tests

        // investor has shares for investment + locking
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(
            lock_amount.val() + invest_amount.val(),
            investor_state.shares.val()
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_locking_twice() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let lock_amount1 = ShareAmount::new(10);
        let lock_amount2 = ShareAmount::new(20);
        // an amount we unlock and will not lock again, to make the test a little more robust
        let invest_amount_not_lock = ShareAmount::new(5);

        let project = create_project_flow(&td).await?;

        // precs

        invests_optins_flow(algod, investor, &project.project).await?;

        // for user to have free shares (assets) to lock
        buy_and_unlock_shares(
            td,
            investor,
            &project.project,
            ShareAmount::new(
                lock_amount1.val() + lock_amount2.val() + invest_amount_not_lock.val(),
            ),
            &project.project_id,
        )
        .await?;

        // flow

        // lock shares
        invests_optins_flow(algod, investor, &project.project).await?; // optin again: unlocking opts user out
        lock_flow(
            algod,
            &project.project,
            &project.project_id,
            &investor,
            lock_amount1,
        )
        .await?;

        // double check: investor has locked shares
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(lock_amount1, investor_state.shares);

        // lock more shares
        lock_flow(
            algod,
            &project.project,
            &project.project_id,
            investor,
            lock_amount2,
        )
        .await?;

        // tests

        // investor has shares for investment + locking
        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        assert_eq!(
            lock_amount1.val() + lock_amount2.val(),
            investor_state.shares.val()
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invest_after_drain_inits_already_harvested_correctly() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;
        let drainer = &investor2();

        let buy_share_amount = ShareAmount::new(10);

        let project = create_project_flow(&td).await?;

        // precs

        // add some funds
        let central_funds = FundsAmount::new(10 * 1_000_000);
        customer_payment_and_drain_flow(td, &project.project, central_funds, drainer).await?;

        invests_optins_flow(algod, investor, &project.project).await?;

        // flow
        invests_flow(
            td,
            investor,
            buy_share_amount,
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
        )?;

        // investing inits the "harvested" amount to entitled amount (to prevent double harvest)
        assert_eq!(investor_entitled_harvest, investor_state.harvested);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_lock_after_drain_inits_already_harvested_correctly() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;
        let drainer = &investor2();

        let buy_share_amount = ShareAmount::new(10);

        let project = create_project_flow(&td).await?;

        // precs

        // add some funds
        let central_funds = FundsAmount::new(10 * 1_000_000);
        customer_payment_and_drain_flow(td, &project.project, central_funds, drainer).await?;

        invests_optins_flow(algod, investor, &project.project).await?;

        // for user to have some free shares (assets) to lock
        buy_and_unlock_shares(
            td,
            investor,
            &project.project,
            buy_share_amount,
            &project.project_id,
        )
        .await?;

        // flow
        invests_optins_flow(algod, investor, &project.project).await?; // optin again: unlocking opts user out
        lock_flow(
            algod,
            &project.project,
            &project.project_id,
            investor,
            buy_share_amount,
        )
        .await?;

        // tests

        let investor_state =
            central_investor_state(algod, &investor.address(), project.project.central_app_id)
                .await?;
        let central_state = central_global_state(algod, project.project.central_app_id).await?;

        let investor_entitled_harvest = calculate_entitled_harvest(
            central_state.received,
            project.project.specs.shares.supply,
            buy_share_amount,
            TESTS_DEFAULT_PRECISION,
            project.project.specs.investors_part(),
        )?;

        // locking inits the "harvested" amount to entitled amount (to prevent double harvest)
        assert_eq!(investor_entitled_harvest, investor_state.harvested);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    #[ignore] // indexer pause
    async fn test_query_my_investment() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);

        let project = create_project_flow(&td).await?;

        // precs

        invests_optins_flow(algod, investor, &project.project).await?;

        // flow

        invests_flow(
            td,
            investor,
            buy_share_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // check that the invested projects query returns the project where the user invested

        // // give time for indexing
        std::thread::sleep(std::time::Duration::from_secs(10));

        let my_invested_projects = my_current_invested_projects(
            algod,
            &td.indexer,
            &investor.address(),
            &td.programs.escrows,
            &td.dao_deps(),
        )
        .await?;

        assert_eq!(1, my_invested_projects.len());
        assert_eq!(project.project_id, my_invested_projects[0].id);
        assert_eq!(project.project, my_invested_projects[0].project);

        Ok(())
    }

    async fn buy_and_unlock_shares(
        td: &TestDeps,
        investor: &Account,
        project: &Project,
        share_amount: ShareAmount,
        project_id: &ProjectId,
    ) -> Result<()> {
        let algod = &td.algod;

        invests_flow(td, investor, share_amount, &project, project_id).await?;
        let unlock_tx_id = unlock_flow(algod, &project, investor, share_amount).await?;
        wait_for_pending_transaction(algod, &unlock_tx_id).await?;
        Ok(())
    }
}

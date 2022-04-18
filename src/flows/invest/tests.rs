#[cfg(test)]
mod tests {
    use crate::flows::claim::claim::claimable_dividend;
    use crate::flows::create_dao::model::Dao;
    use crate::flows::create_dao::share_amount::ShareAmount;
    use crate::funds::FundsAmount;
    use crate::network_util::wait_for_pending_transaction;
    use crate::queries::my_daos::my_current_invested_daos;
    use crate::state::account_state::{
        asset_holdings, find_asset_holding_or_err, funds_holdings, funds_holdings_from_account,
    };
    use crate::state::dao_app_state::{
        central_investor_state_from_acc, dao_global_state, dao_investor_state,
    };
    use crate::testing::flow::create_dao_flow::create_dao_flow;
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::flow::invest_in_dao_flow::{invests_flow, invests_optins_flow};
    use crate::testing::flow::lock_flow::lock_flow;
    use crate::testing::flow::unlock_flow::unlock_flow;
    use crate::testing::network_test_util::{test_dao_init, TestDeps};
    use crate::testing::test_data::dao_specs;
    use crate::testing::test_data::investor2;
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
        let specs = dao_specs();

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &dao).await?;

        // flow

        let flow_res = invests_flow(&td, &investor, buy_share_amount, &dao).await?;

        // app escrow tests

        // app escrow received the shares
        let app_shares = asset_holdings(algod, &dao.app_address(), dao.shares_asset_id).await?;
        assert_eq!(buy_share_amount.0, app_shares.0);
        // app escrow doesn't send any transactions so not testing balances (we could "double check" though)

        // investor tests

        let investor_infos = algod.account_information(&investor.address()).await?;
        let central_investor_state = central_investor_state_from_acc(&investor_infos, dao.app_id)?;

        // investor has shares
        assert_eq!(buy_share_amount, central_investor_state.shares);

        // check that the dao id was initialized
        assert_eq!(dao.id(), central_investor_state.dao_id);

        // check that claimed is 0 (nothing claimed yet)
        assert_eq!(FundsAmount::new(0), central_investor_state.claimed);
        assert_eq!(FundsAmount::new(0), central_investor_state.claimed_init);

        // double check: investor didn't receive any shares

        let investor_assets = investor_infos.assets.clone();
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // investor lost algos and fees
        let investor_holdings = funds_holdings_from_account(&investor_infos, td.funds_asset_id)?;
        let paid_amount = specs.share_price.val() * buy_share_amount.val();
        assert_eq!(
            flow_res.investor_initial_amount - paid_amount,
            investor_holdings
        );

        // invest escrow tests

        let invest_escrow = flow_res.dao.invest_escrow;
        let invest_escrow_infos = algod.account_information(invest_escrow.address()).await?;
        let invest_escrow_held_assets = invest_escrow_infos.assets;
        // investing escrow lost the bought assets
        assert_eq!(invest_escrow_held_assets.len(), 1);
        assert_eq!(
            invest_escrow_held_assets[0].asset_id,
            flow_res.dao.shares_asset_id
        );
        assert_eq!(
            invest_escrow_held_assets[0].amount,
            flow_res.dao.specs.shares.supply.val() - buy_share_amount.val()
        );

        // central escrow tests

        // central escrow received paid algos
        let app_holdings = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        assert_eq!(
            flow_res.central_escrow_initial_amount + paid_amount,
            app_holdings
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

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(&algod, &investor, &dao).await?;

        // flow

        invests_flow(td, investor, buy_share_amount, &dao).await?;

        // double check: investor has shares for first investment
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(buy_share_amount, investor_state.shares);

        invests_flow(td, investor, buy_share_amount2, &dao).await?;

        // tests

        // investor has shares for both investments
        let investor_state = dao_investor_state(&algod, &investor.address(), dao.app_id).await?;
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

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(algod, investor, &dao).await?;

        // for user to have some free shares (assets) to lock
        buy_and_unlock_shares(td, investor, &dao, lock_amount).await?;

        // flow

        // buy shares: automatically locked
        invests_optins_flow(algod, investor, &dao).await?; // optin again: unlocking opts user out
        invests_flow(td, investor, invest_amount, &dao).await?;

        // double check: investor has shares for first investment
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(invest_amount, investor_state.shares);

        // lock shares
        lock_flow(algod, &dao, investor, lock_amount).await?;

        // tests

        // investor has shares for investment + locking
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
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

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &dao).await?;

        // for user to have some free shares (assets) to lock
        buy_and_unlock_shares(td, investor, &dao, lock_amount).await?;

        // flow

        // lock shares
        invests_optins_flow(algod, investor, &dao).await?; // optin again: unlocking opts user out
        lock_flow(&algod, &dao, &investor, lock_amount).await?;

        // double check: investor has locked shares
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(lock_amount, investor_state.shares);

        // buy shares: automatically locked
        invests_flow(td, investor, invest_amount, &dao).await?;

        // tests

        // investor has shares for investment + locking
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
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

        let dao = create_dao_flow(&td).await?;

        // precs

        invests_optins_flow(algod, investor, &dao).await?;

        // for user to have free shares (assets) to lock
        buy_and_unlock_shares(
            td,
            investor,
            &dao,
            ShareAmount::new(
                lock_amount1.val() + lock_amount2.val() + invest_amount_not_lock.val(),
            ),
        )
        .await?;

        // flow

        // lock shares
        invests_optins_flow(algod, investor, &dao).await?; // optin again: unlocking opts user out
        lock_flow(algod, &dao, &investor, lock_amount1).await?;

        // double check: investor has locked shares
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(lock_amount1, investor_state.shares);

        // lock more shares
        lock_flow(algod, &dao, investor, lock_amount2).await?;

        // tests

        // investor has shares for investment + locking
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(
            lock_amount1.val() + lock_amount2.val(),
            investor_state.shares.val()
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invest_after_drain_inits_already_claimed_correctly() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;
        let drainer = &investor2();

        let buy_share_amount = ShareAmount::new(10);

        let dao = create_dao_flow(&td).await?;

        // precs

        // add some funds
        let central_funds = FundsAmount::new(10 * 1_000_000);
        customer_payment_and_drain_flow(td, &dao, central_funds, drainer).await?;

        invests_optins_flow(algod, investor, &dao).await?;

        // flow
        invests_flow(td, investor, buy_share_amount, &dao).await?;

        // tests

        let investor_state = dao_investor_state(&algod, &investor.address(), dao.app_id).await?;
        let central_state = dao_global_state(&algod, dao.app_id).await?;

        let claimable_dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            dao.specs.shares.supply,
            buy_share_amount,
            td.precision,
            dao.specs.investors_part(),
        )?;

        // investing inits the "claimed" amount to entitled amount (to prevent double claiming)
        assert_eq!(claimable_dividend, investor_state.claimed);
        // claimed_init is initialized to the entitled amount too
        assert_eq!(claimable_dividend, investor_state.claimed_init);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_lock_after_drain_inits_already_claimed_correctly() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;
        let drainer = &investor2();

        let buy_share_amount = ShareAmount::new(10);

        let dao = create_dao_flow(&td).await?;

        // precs

        // add some funds
        let central_funds = FundsAmount::new(10 * 1_000_000);
        customer_payment_and_drain_flow(td, &dao, central_funds, drainer).await?;

        invests_optins_flow(algod, investor, &dao).await?;

        // for user to have some free shares (assets) to lock
        buy_and_unlock_shares(td, investor, &dao, buy_share_amount).await?;

        // flow
        invests_optins_flow(algod, investor, &dao).await?; // optin again: unlocking opts user out
        lock_flow(algod, &dao, investor, buy_share_amount).await?;

        // tests

        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        let central_state = dao_global_state(algod, dao.app_id).await?;

        let claimable_dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            dao.specs.shares.supply,
            buy_share_amount,
            td.precision,
            dao.specs.investors_part(),
        )?;

        // locking inits the "claimed" amount to entitled amount (to prevent double claiming)
        assert_eq!(claimable_dividend, investor_state.claimed);
        // claimed_init is initialized to the entitled amount too
        assert_eq!(claimable_dividend, investor_state.claimed_init);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_query_my_investment() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);

        let dao = create_dao_flow(&td).await?;

        // precs

        invests_optins_flow(algod, investor, &dao).await?;

        // flow

        invests_flow(td, investor, buy_share_amount, &dao).await?;

        // test

        let my_invested_daos =
            my_current_invested_daos(algod, &investor.address(), td.api.as_ref(), &td.dao_deps())
                .await?;

        assert_eq!(1, my_invested_daos.len());
        assert_eq!(dao.id(), my_invested_daos[0].id());
        assert_eq!(dao, my_invested_daos[0]);

        Ok(())
    }

    async fn buy_and_unlock_shares(
        td: &TestDeps,
        investor: &Account,
        dao: &Dao,
        share_amount: ShareAmount,
    ) -> Result<()> {
        let algod = &td.algod;

        invests_flow(td, investor, share_amount, &dao).await?;
        let unlock_tx_id = unlock_flow(algod, &dao, investor, dao.shares_asset_id).await?;
        wait_for_pending_transaction(algod, &unlock_tx_id).await?;
        Ok(())
    }
}

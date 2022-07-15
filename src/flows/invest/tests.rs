#[cfg(test)]
mod tests {
    use crate::flows::claim::claim::claimable_dividend;
    use crate::flows::create_dao::model::Dao;
    use crate::flows::create_dao::storage::load_dao::load_dao;
    use crate::network_util::wait_for_pending_transaction;
    use crate::queries::my_daos::my_current_invested_daos;
    use crate::state::account_state::{
        asset_holdings, find_asset_holding_or_err, funds_holdings, funds_holdings_from_account,
    };
    use crate::state::dao_shares::dao_shares;
    use crate::testing::flow::create_dao_flow::create_dao_flow;
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::flow::invest_in_dao_flow::{
        invests_flow, invests_optins_flow, InvestInDaoTestFlowRes,
    };
    use crate::testing::flow::lock_flow::lock_flow;
    use crate::testing::flow::unlock_flow::unlock_flow;
    use crate::testing::network_test_util::{test_dao_init, TestDeps};
    use crate::testing::test_data::investor2;
    use algonaut::transaction::account::Account;
    use anyhow::Result;
    use mbase::checked::{CheckedAdd, CheckedSub};
    use mbase::models::funds::FundsAmount;
    use mbase::models::share_amount::ShareAmount;
    use mbase::state::dao_app_state::{
        central_investor_state_from_acc, dao_global_state, dao_investor_state,
    };
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invests_flow() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &dao).await?;

        // flow

        let flow_res = invests_flow(&td, &investor, buy_share_amount, &dao).await?;

        // tests

        // check that claimed is 0 (nothing claimed yet)
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_state = central_investor_state_from_acc(&investor_infos, dao.app_id)?;
        assert_eq!(FundsAmount::new(0), investor_state.claimed);
        assert_eq!(FundsAmount::new(0), investor_state.claimed_init);

        test_simple_investment_res(&td, &dao, investor, buy_share_amount, flow_res).await
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

        let total_shares = ShareAmount::new(buy_share_amount.val() + buy_share_amount2.val());

        // locked shares global state set to bought shares
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(total_shares, gs.locked_shares);

        // investor has shares for both investments
        let investor_state = dao_investor_state(&algod, &investor.address(), dao.app_id).await?;
        assert_eq!(total_shares, investor_state.shares);

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

        let total_shares = ShareAmount::new(lock_amount.val() + invest_amount.val());

        // locked shares global state set to investment + locking
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(total_shares, gs.locked_shares);

        // investor has shares for investment + locking
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(total_shares, investor_state.shares);

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

        let total_shares = ShareAmount::new(lock_amount.val() + invest_amount.val());

        // locked shares global state set to investment + locking
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(total_shares, gs.locked_shares);

        // investor has shares for investment + locking
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(total_shares, investor_state.shares);

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

        let total_shares = ShareAmount::new(lock_amount1.val() + lock_amount2.val());

        // locked shares global state set to total
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(total_shares, gs.locked_shares);

        // investor has shares for investment + locking
        let investor_state = dao_investor_state(algod, &investor.address(), dao.app_id).await?;
        assert_eq!(total_shares, investor_state.shares);

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
            dao.token_supply,
            buy_share_amount,
            td.precision,
            dao.investors_share,
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
            dao.token_supply,
            buy_share_amount,
            td.precision,
            dao.investors_share,
        )?;

        // locking inits the "claimed" amount to entitled amount (to prevent double claiming)
        assert_eq!(claimable_dividend, investor_state.claimed);
        // claimed_init is initialized to the entitled amount too
        assert_eq!(claimable_dividend, investor_state.claimed_init);

        Ok(())
    }

    // TODO test (other file) investing with price that doesn't lead to a whole share amount (to check that TEAL floors it correctly / rejects if amount is wrong)
    // these are not happy path tests - thus different test location - our app prevents sending this (paid price is derived from bought shares)

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

        // fetch dao again, because investing changes raised field
        let dao = load_dao(algod, dao.id()).await?;

        let my_invested_daos = my_current_invested_daos(algod, &investor.address()).await?;

        assert_eq!(1, my_invested_daos.len());
        assert_eq!(dao.id(), my_invested_daos[0].id());
        assert_eq!(dao, my_invested_daos[0]);

        Ok(())
    }

    // capi fees and dividends are generated only by consumer payments - investment income shouldn't have any effect there
    // note that in the future we may raise capi fees on investments too
    #[test]
    #[serial] // reset network (cmd)
    async fn test_investment_does_not_generate_capi_fees_or_dividends() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(&algod, &investor, &dao).await?;

        // flow

        let flow_res = invests_flow(td, investor, buy_share_amount, &dao).await?;

        // test

        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        // investment income is immediately put in withdrawable global state
        // NOTE that the funds may still *not* be actually withdrawable, if withdrawing before funds raising end date
        assert_eq!(flow_res.total_paid_price, dao_state.available);

        // no dividend: so total received (which is used to calculate the dividend) was not incremented
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(FundsAmount::new(0), dao_state.received);

        // sanity: regular investment tests

        // check that claimed is 0 (nothing claimed yet)
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_state = central_investor_state_from_acc(&investor_infos, dao.app_id)?;
        assert_eq!(FundsAmount::new(0), investor_state.claimed);
        assert_eq!(FundsAmount::new(0), investor_state.claimed_init);

        test_simple_investment_res(&td, &dao, investor, buy_share_amount, flow_res).await
    }

    // for basic explanation, see test_investment_does_not_generate_capi_fees_or_dividends
    // this tests invests *after* draining - we expect the same resulting state
    // note that this use case may not exist in practice,
    // as probably (especially for legal reasons) the funds raising phase has to be finished,
    // before being able to accept money from customers.
    // But it's technically possible in any case.
    #[test]
    #[serial] // reset network (cmd)
    async fn test_investment_after_draining_does_not_generate_capi_fees_or_dividends() -> Result<()>
    {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;
        let drainer = &td.investor2;

        let buy_share_amount = ShareAmount::new(10);

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(&algod, &investor, &dao).await?;

        // flow

        // drain
        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);
        let drain_res =
            customer_payment_and_drain_flow(&td, &dao, customer_payment_amount, drainer).await?;

        // invest
        let invest_res = invests_flow(td, investor, buy_share_amount, &dao).await?;

        // test

        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        // investment income is immediately put in withdrawable global state
        // we drained too, so we expect the withdrawable state to be the investment + drained amount
        // NOTE that the funds may still *not* be actually withdrawable, if withdrawing before funds raising end date
        let expected_withdrawable_funds = FundsAmount::new(
            drain_res.drained_amounts.dao.val() + invest_res.total_paid_price.val(),
        );
        assert_eq!(expected_withdrawable_funds, dao_state.available);

        // total received (which is used to calculate the dividend) was incremented by the drained amount (and not by the invest amount)
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(drain_res.drained_amounts.dao, dao_state.received);

        // sanity: regular investment tests
        // note that we skip testing for claimed state here,
        // as we'd have to calculate the dividend for the drained amount and that seems out of scope here (investment tests)
        test_simple_investment_res(&td, &dao, investor, buy_share_amount, invest_res).await
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_canot_invest_if_not_enough_shares_supply() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        // buy more than supply
        let buy_share_amount = ShareAmount::new(td.specs.shares.supply.val() + 1);

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &dao).await?;

        // flow

        let flow_res = invests_flow(&td, &investor, buy_share_amount, &dao).await;

        // tests

        assert!(flow_res.is_err());

        Ok(())
    }

    // can't invest more than amount reserved for investors
    // note that this test assumes that it's run after initializing dao,
    // in real world it's technically possible that the dao has more shares than initially reserved to investors
    // (the dao owner can transfer their shares to the dao for whatever reason)
    #[test]
    #[serial] // reset network (cmd)
    async fn test_canot_invest_if_not_enough_shares_for_investors() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let dao = create_dao_flow(td).await?;

        // precs

        invests_optins_flow(algod, &investor, &dao).await?;

        // flow

        let buy_share_amount = ShareAmount::new(td.specs.shares_for_investors().val() + 1);
        let flow_res = invests_flow(&td, &investor, buy_share_amount, &dao).await;

        // tests

        assert!(flow_res.is_err());

        Ok(())
    }

    /// here there's enough supply / shares reserved for investors, but not enough avaiable (not locked)
    /// meaning: other investors have exhausted the available supply
    #[test]
    #[serial] // reset network (cmd)
    async fn test_canot_invest_if_not_enough_shares_for_sale() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let dao = create_dao_flow(td).await?;

        // precs

        // have someone buy almost all the avaiable supply
        assert!(td.specs.shares.supply.val() > 1); // sanity check
        let buy_share_amount = ShareAmount::new(td.specs.shares_for_investors().val() - 1);
        invests_optins_flow(algod, &investor, &dao).await?;
        let flow_res = invests_flow(&td, &investor, buy_share_amount, &dao).await;
        assert!(flow_res.is_ok());

        // flow

        // buy more than available (previous bought all minus 1, so there's only 1 remaining)
        // we could make previous buy everything as well, but minus 1 seems more sensible to catch issues
        let buy_share_amount = ShareAmount::new(2);
        let flow_res = invests_flow(&td, &investor, buy_share_amount, &dao).await;

        // tests

        assert!(flow_res.is_err());

        Ok(())
    }

    async fn test_simple_investment_res(
        td: &TestDeps,
        dao: &Dao,
        investor: &Account,
        buy_share_amount: ShareAmount,
        flow_res: InvestInDaoTestFlowRes,
    ) -> Result<()> {
        let algod = &td.algod;

        // locked shares global state set to bought shares
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(buy_share_amount, gs.locked_shares);

        // app escrow still has all the shares
        let app_shares =
            ShareAmount(asset_holdings(algod, &dao.app_address(), dao.shares_asset_id).await?);
        assert_eq!(td.specs.shares_for_investors(), app_shares);

        let investor_infos = algod.account_information(&investor.address()).await?;
        let central_investor_state = central_investor_state_from_acc(&investor_infos, dao.app_id)?;

        // investor has shares
        assert_eq!(buy_share_amount, central_investor_state.shares);

        // double check: investor didn't receive any shares

        let investor_assets = investor_infos.assets.clone();
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // investor lost algos and fees
        let investor_holdings = funds_holdings_from_account(&investor_infos, td.funds_asset_id)?;
        let paid_amount = FundsAmount::new(td.specs.share_price.val() * buy_share_amount.val());
        assert_eq!(
            flow_res.investor_initial_amount.sub(&paid_amount).unwrap(),
            investor_holdings
        );

        // app escrow tests

        let app_holdings = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        // app escrow received paid algos
        assert_eq!(
            flow_res
                .central_escrow_initial_amount
                .add(&paid_amount)
                .unwrap(),
            app_holdings
        );
        let dao_shares = dao_shares(algod, dao.app_id, dao.shares_asset_id).await?;
        assert_eq!(buy_share_amount, dao_shares.locked); // bought shares added to locked shares
        assert_eq!(
            ShareAmount::new(td.specs.shares_for_investors().val() - buy_share_amount.val()),
            dao_shares.available
        ); // bought shares subtracted from available shares

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
        let unlock_tx_id = unlock_flow(algod, &dao, investor).await?;
        wait_for_pending_transaction(algod, &unlock_tx_id).await?;
        Ok(())
    }
}

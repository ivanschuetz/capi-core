#[cfg(test)]
mod tests {
    use crate::{
        flows::claim::claim::claimable_dividend,
        state::account_state::funds_holdings,
        testing::{
            flow::{
                claim_flow::{claim_flow, claim_precs},
                invest_in_dao_flow::invests_flow,
            },
            network_test_util::{test_dao_init, TestDeps},
        },
    };
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use mbase::{
        checked::CheckedAdd,
        checked::CheckedSub,
        models::{
            create_shares_specs::CreateSharesSpecs,
            dao_app_id::DaoAppId,
            funds::{FundsAmount, FundsAssetId},
            setup_dao_specs::SetupDaoSpecs,
            share_amount::ShareAmount,
        },
        state::dao_app_state::{
            central_investor_state_from_acc, dao_global_state, dao_investor_state,
            CentralAppGlobalState,
        },
    };
    use rust_decimal::Decimal;
    use serial_test::serial;
    use std::{convert::TryInto, str::FromStr};
    use tokio::test;

    #[test]
    #[serial]
    async fn test_claim_max() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let claimer = &td.investor2;

        // flow

        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);

        let precs = claim_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            drainer,
            claimer,
        )
        .await?;

        let central_state_before_claim = dao_global_state(&algod, precs.dao.app_id).await?;
        let res = claim_flow(&td, &precs.dao, claimer).await?;

        // test

        let dividend = claimable_dividend(
            precs.drain_res.drained_amounts.dao,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_share,
        )?;

        test_claim_result(
            &algod,
            &claimer,
            res.dao.app_id,
            td.funds_asset_id,
            precs.drain_res.drained_amounts.dao,
            // claimer got the amount
            res.claimer_balance_before_claiming.add(&dividend).unwrap(),
            // central lost the amount
            precs.app_balance_after_drain.sub(&dividend).unwrap(),
            // double check shares local state
            buy_share_amount,
            // only one dividend: local state is the claimed amount
            dividend,
            // invested before first drain: initial entitled dividend is 0
            FundsAmount::new(0),
            &central_state_before_claim,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_claim_max_with_repeated_fractional_shares_percentage() -> Result<()> {
        let td = test_fractional_deps().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let claimer = &td.investor2;

        // precs

        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        // 10 shares, 300 supply, 100% investor's share, percentage: 0.0333333333

        let precs = claim_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            &drainer,
            &claimer,
        )
        .await?;

        let central_state_before_claim = dao_global_state(&algod, precs.dao.app_id).await?;
        log::debug!(
            "central_total_received: {:?}",
            central_state_before_claim.received
        );

        // flow

        let res = claim_flow(&td, &precs.dao, &claimer).await?;

        // test

        let dividend = claimable_dividend(
            central_state_before_claim.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_share,
        )?;
        log::debug!("dividend: {}", dividend);

        test_claim_result(
            &algod,
            &claimer,
            res.dao.app_id,
            td.funds_asset_id,
            precs.drain_res.drained_amounts.dao,
            // claimer got the amount
            res.claimer_balance_before_claiming.add(&dividend).unwrap(),
            // central lost the amount
            precs.app_balance_after_drain.sub(&dividend).unwrap(),
            // double check shares local state
            buy_share_amount,
            // only one claim: local state is the claimed amount
            dividend,
            // invested before first drain: initial entitled dividend is 0
            FundsAmount::new(0),
            &central_state_before_claim,
        )
        .await?;

        Ok(())
    }

    async fn test_fractional_deps() -> Result<TestDeps> {
        let mut td = test_dao_init().await?;
        // set capi percentage to 0 - we're not testing this here and it eases calculations (drained amount == amount that ends on central escrow)
        td.capi_escrow_percentage = Decimal::new(0, 0).try_into().unwrap();
        td.specs = SetupDaoSpecs::new(
            "Pancakes ltd".to_owned(),
            None,
            CreateSharesSpecs {
                token_name: "PCK".to_owned(),
                supply: ShareAmount::new(300),
            },
            Decimal::from_str("0.4")?.try_into()?,
            FundsAmount::new(5_000_000),
            None,
            "https://twitter.com/helloworld".to_owned(),
            "https://helloworld.com".to_owned(),
            ShareAmount::new(250), // assumes a higher supply number
            FundsAmount::new(0), // 0 target means practically no target - we'll use different deps to test funds target
            (Utc::now() - Duration::minutes(1)).into(), // in the past means practically no funds raising period - we'll use different deps to test funds target
            None,
            ShareAmount::new(0),
            ShareAmount::new(u64::MAX),
        )?;
        Ok(td)
    }

    #[test]
    #[serial]
    async fn test_2_successful_claims() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let claimer = &td.investor2;

        // flow 1

        let buy_share_amount = ShareAmount::new(20);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);

        let precs = claim_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            &drainer,
            &claimer,
        )
        .await?;

        let central_state_before_claim = dao_global_state(&algod, precs.dao.app_id).await?;
        let dividend = claimable_dividend(
            central_state_before_claim.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_share,
        )?;
        let res1 = claim_flow(&td, &precs.dao, &claimer).await?;

        // test 1

        test_claim_result(
            &algod,
            &claimer,
            res1.dao.app_id,
            td.funds_asset_id,
            precs.drain_res.drained_amounts.dao,
            // asset balance is the claimed amount
            res1.claimer_balance_before_claiming.add(&dividend).unwrap(),
            // central lost the amount
            precs.app_balance_after_drain.sub(&dividend).unwrap(),
            // double check shares local state
            buy_share_amount,
            // local state is the claimed amount
            dividend,
            // invested before first drain: initial entitled dividend is 0
            FundsAmount::new(0),
            &central_state_before_claim,
        )
        .await?;

        // flow 2

        let _ = claim_flow(&td, &precs.dao, &claimer).await?;

        // bonus condition: no-op claim: nothing new has been drained, we claim 0, which doesn't have any effect
        // (the test also passes with this line commented)
        let _ = claim_flow(&td, &precs.dao, &claimer).await?;

        // test 2

        test_claim_result(
            &algod,
            &claimer,
            res1.dao.app_id,
            td.funds_asset_id,
            precs.drain_res.drained_amounts.dao,
            res1.claimer_balance_before_claiming.add(&dividend).unwrap(),
            precs.app_balance_after_drain.sub(&dividend).unwrap(),
            buy_share_amount,
            dividend,
            FundsAmount::new(0),
            &central_state_before_claim,
        )
        .await?;

        Ok(())
    }

    // TODO like test_2_successful_claims but not enough funds for 2nd claim
    // (was accidentally partly tested with test_2_successful_claims, as the default accounts didn't have enough funds for the 2nd claim,
    // but should be a permanent test of course)

    async fn test_claim_result(
        algod: &Algod,
        claimer: &Account,
        app_id: DaoAppId,
        funds_asset_id: FundsAssetId,
        // this parameter isn't ideal: it assumes that we did a (one) drain before claiming
        // for now letting it there as it's a quick refactoring
        // arguably needed, it tests basically that the total received global state isn't affected by claiming
        // (otherwise this is/should be already tested in the drain logic)
        drained_amount: FundsAmount,
        expected_claimer_balance: FundsAmount,
        expected_central_balance: FundsAmount,
        expected_shares: ShareAmount,
        expected_claimed_total: FundsAmount,
        expected_claimed_init: FundsAmount,
        state_before_claiming: &CentralAppGlobalState,
    ) -> Result<()> {
        let claim_funds_amount = funds_holdings(algod, &claimer.address(), funds_asset_id).await?;
        let central_escrow_funds_amount =
            funds_holdings(algod, &app_id.address(), funds_asset_id).await?;

        assert_eq!(expected_claimer_balance, claim_funds_amount);
        assert_eq!(expected_central_balance, central_escrow_funds_amount);

        // the total received didn't change
        // (i.e. same as expected after draining, claiming doesn't affect it)
        let global_state = dao_global_state(algod, app_id).await?;
        assert_eq!(global_state.received, drained_amount);

        // claimer local state: test that it was incremented by amount claimed
        // Only one local variable used
        let claimer_account = algod.account_information(&claimer.address()).await?;
        assert_eq!(1, claimer_account.apps_local_state.len());

        // check local state

        let investor_state = central_investor_state_from_acc(&claimer_account, app_id)?;

        // double-check shares count (not directly related to this test)
        assert_eq!(expected_shares, investor_state.shares);
        // check claimed local state
        assert_eq!(expected_claimed_total, investor_state.claimed);
        assert_eq!(expected_claimed_init, investor_state.claimed_init);

        // check that withdrawable amount was decreased by claimed amount
        assert_eq!(
            FundsAmount::new(state_before_claiming.available.val() - investor_state.claimed.val()),
            global_state.available
        );

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_pending_dividend_preserved_when_locking_again() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let claimer = &td.investor2;

        // precs

        // invests_optins_flow(algod, &claimer, &precs.dao).await?;
        // invests_flow(&td, &claimer, ShareAmount::new(10), &precs.dao).await?;

        let buy_share_amount = ShareAmount::new(20);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);

        let precs = claim_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            &drainer,
            &claimer,
        )
        .await?;

        // tests

        // double-check investor state before investing again

        let investor_state =
            dao_investor_state(algod, &claimer.address(), precs.dao.app_id).await?;

        let central_state = dao_global_state(&algod, precs.dao.app_id).await?;
        // this is the claimable dividend at this point - "unclaimed" because investor never claims it
        let unclaimed_dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_share,
        )?;

        // log::debug!("{pending_dividend:?}");

        // there was income and drain while invested, so investor has something to claim
        assert!(unclaimed_dividend.val() > 0);

        // the shares we just bought are in state
        assert_eq!(buy_share_amount, investor_state.shares);
        // invested before the dao ever drained, so claimed_init initialized to 0
        assert_eq!(FundsAmount::new(0), investor_state.claimed_init);
        // investor hasn't claimed anything yet, so 0
        assert_eq!(FundsAmount::new(0), investor_state.claimed);

        // investor has unclaimed dividend now

        // investor buys more shares
        let new_buy_amount = ShareAmount::new(10);
        invests_flow(&td, &claimer, new_buy_amount, &precs.dao).await?;

        let investor_state =
            dao_investor_state(algod, &claimer.address(), precs.dao.app_id).await?;

        // new total locked shares (investing locks the shares automatically)
        let total_locked_shares = ShareAmount::new(buy_share_amount.val() + new_buy_amount.val());

        let central_state = dao_global_state(&algod, precs.dao.app_id).await?;
        let dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            total_locked_shares,
            td.precision,
            td.specs.investors_share,
        )?;
        // test that claimed_init is initialized to claimable dividend (based on total number of shares) MINUS unclaimed dividend
        // the reasoning here is that we initalize "already claimed" in teal to what the investor is entitled to when locking the shares,
        // to ensure dividend can be claimed only for the future
        // and we substract dividend that hasn't been claimed yet, to prevent buying/locking of new shares removing not claimed dividend.
        let expected_claim_init = FundsAmount::new(dividend.val() - unclaimed_dividend.val());
        assert_eq!(expected_claim_init, investor_state.claimed_init);
        // when locking shares, claimed is initialized to claim_init, so we expect it to have the same value
        assert_eq!(expected_claim_init, investor_state.claimed);

        Ok(())
    }

    // TODO test: can't claim not available amount
}

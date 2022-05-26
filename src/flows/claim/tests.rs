#[cfg(test)]
mod tests {
    use std::{convert::TryInto, str::FromStr};
    use crate::{
        flows::{
            claim::claim::claimable_dividend,
            create_dao::{
                model::CreateSharesSpecs, setup_dao_specs::{SetupDaoSpecs},
            },
        },
        state::{
            account_state::funds_holdings,
        },
        testing::{
            flow::claim_flow::{claim_flow, claim_precs},
            network_test_util::{test_dao_init, TestDeps},
        },
    };
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::Result;
    use chrono::{Utc, Duration};
    use mbase::{models::{share_amount::ShareAmount, funds::{FundsAmount, FundsAssetId}, hash::ImageHash, dao_app_id::DaoAppId}, state::dao_app_state::{dao_global_state, central_investor_state_from_acc}, api::version::VersionedAddress};
    use rust_decimal::Decimal;
    use serial_test::serial;
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
            &res.dao.customer_escrow.to_versioned_address(),
            precs.drain_res.drained_amounts.dao,
            // claimer got the amount
            res.claimer_balance_before_claiming + dividend,
            // central lost the amount
            precs.app_balance_after_drain - dividend,
            // double check shares local state
            buy_share_amount,
            // only one dividend: local state is the claimed amount
            dividend,
            // invested before first drain: initial entitled dividend is 0
            FundsAmount::new(0),
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

        let central_state = dao_global_state(&algod, precs.dao.app_id).await?;
        log::debug!("central_total_received: {:?}", central_state.received);

        // flow

        let res = claim_flow(&td, &precs.dao, &claimer).await?;

        // test

        let dividend = claimable_dividend(
            central_state.received,
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
            &res.dao.customer_escrow.to_versioned_address(),
            precs.drain_res.drained_amounts.dao,
            // claimer got the amount
            res.claimer_balance_before_claiming + dividend,
            // central lost the amount
            precs.app_balance_after_drain - dividend,
            // double check shares local state
            buy_share_amount,
            // only one claim: local state is the claimed amount
            dividend,
            // invested before first drain: initial entitled dividend is 0
            FundsAmount::new(0),
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
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore".to_owned(), 
            CreateSharesSpecs { 
                token_name: "PCK".to_owned(), 
                supply: ShareAmount::new(300),
            },
            Decimal::from_str("0.4")?.try_into()?,
            FundsAmount::new(5_000_000),
            Some(ImageHash("test_hash".to_owned())),
            "https://twitter.com/capi_fin".to_owned(),
            ShareAmount::new(250), // assumes a higher supply number
            FundsAmount::new(0), // 0 target means practically no target - we'll use different deps to test funds target
            (Utc::now() - Duration::minutes(1)).into() // in the past means practically no funds raising period - we'll use different deps to test funds target
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

        // test 1

        let central_state = dao_global_state(&algod, precs.dao.app_id).await?;

        let dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_share,
        )?;
        let res1 = claim_flow(&td, &precs.dao, &claimer).await?;

        test_claim_result(
            &algod,
            &claimer,
            res1.dao.app_id,
            td.funds_asset_id,
            &res1.dao.customer_escrow.to_versioned_address(),
            precs.drain_res.drained_amounts.dao,
            // asset balance is the claimed amount
            res1.claimer_balance_before_claiming + dividend,
            // central lost the amount
            precs.app_balance_after_drain - dividend,
            // double check shares local state
            buy_share_amount,
            // local state is the claimed amount
            dividend,
            // invested before first drain: initial entitled dividend is 0
            FundsAmount::new(0),
        )
        .await?;

        // flow 2

        let _ = claim_flow(&td, &precs.dao, &claimer).await?;

        // test 2
        // nothing new has been drained, we claim 0, and test that all the state is still the same

        let _ = claim_flow(&td, &precs.dao, &claimer).await?;

        test_claim_result(
            &algod,
            &claimer,
            res1.dao.app_id,
            td.funds_asset_id,
            &res1.dao.customer_escrow.to_versioned_address(),
            precs.drain_res.drained_amounts.dao,
            res1.claimer_balance_before_claiming + dividend,
            precs.app_balance_after_drain - dividend,
            buy_share_amount,
            dividend,
            FundsAmount::new(0),
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
        customer_escrow_address: &VersionedAddress,
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

        // sanity check: global state addresses are set
        assert_eq!(&global_state.customer_escrow, customer_escrow_address);

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

        Ok(())
    }
}

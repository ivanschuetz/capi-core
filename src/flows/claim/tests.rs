#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use crate::{
        flows::{
            claim::claim::claimable_dividend,
            create_dao::{
                create_dao_specs::CreateDaoSpecs, model::CreateSharesSpecs,
                share_amount::ShareAmount,
            },
        },
        funds::{FundsAmount, FundsAssetId},
        state::{
            account_state::funds_holdings,
            central_app_state::{central_global_state, central_investor_state_from_acc},
        },
        testing::{
            flow::claim_flow::{claim_flow, claim_precs},
            network_test_util::{test_dao_init, TestDeps},
        },
    };
    use algonaut::{algod::v2::Algod, core::Address, transaction::account::Account};
    use anyhow::Result;
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

        let dividend = claimable_dividend(
            precs.drain_res.drained_amounts.dao,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_part(),
        )?;

        let res = claim_flow(&td, &precs.dao, claimer, dividend).await?;

        // test

        test_claim_result(
            &algod,
            &claimer,
            res.dao.central_app_id,
            td.funds_asset_id,
            res.dao.central_escrow.address(),
            res.dao.customer_escrow.address(),
            precs.drain_res.drained_amounts.dao,
            // claimer got the amount
            res.claimer_balance_before_claiming + res.claimed,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.claimed,
            // double check shares local state
            buy_share_amount,
            // only one dividend: local state is the claimed amount
            res.claimed,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_claim_more_than_max() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let drainer = &td.investor1;
        let claimer = &td.investor2;

        // precs

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

        let central_state = central_global_state(&algod, precs.dao.central_app_id).await?;
        let claim_amount = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_part(),
        )?;
        log::debug!("Claim amount: {}", claim_amount);

        // flow

        // we claim 1 asset more than max allowed
        let res = claim_flow(&td, &precs.dao, &claimer, claim_amount + 1).await;
        log::debug!("res: {:?}", res);

        // test

        assert!(res.is_err());

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

        let central_state = central_global_state(&algod, precs.dao.central_app_id).await?;
        log::debug!("central_total_received: {:?}", central_state.received);

        let dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_part(),
        )?;
        log::debug!("dividend: {}", dividend);

        // flow

        let res = claim_flow(&td, &precs.dao, &claimer, dividend).await?;

        // test

        test_claim_result(
            &algod,
            &claimer,
            res.dao.central_app_id,
            td.funds_asset_id,
            res.dao.central_escrow.address(),
            res.dao.customer_escrow.address(),
            precs.drain_res.drained_amounts.dao,
            // claimer got the amount
            res.claimer_balance_before_claiming + res.claimed,
            // central lost the amount
            precs.central_escrow_balance_after_drain - res.claimed,
            // double check shares local state
            buy_share_amount,
            // only one claim: local state is the claimed amount
            res.claimed,
        )
        .await?;

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_claim_max_with_repeated_fractional_shares_percentage_plus_1_fails() -> Result<()>
    {
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

        let central_state = central_global_state(&algod, precs.dao.central_app_id).await?;
        log::debug!("central_total_received: {:?}", central_state.received);

        let dividend = claimable_dividend(
            central_state.received,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_part(),
        )?;
        log::debug!("Dividend: {}", dividend);

        // flow

        // The claimable dividend calculation and TEAL use floor to round the decimal. TEAL will reject + 1
        let res = claim_flow(&td, &precs.dao, &claimer, dividend + 1).await;

        // test

        assert!(res.is_err());

        Ok(())
    }

    async fn test_fractional_deps() -> Result<TestDeps> {
        let mut td = test_dao_init().await?;
        // set capi percentage to 0 - we're not testing this here and it eases calculations (drained amount == amount that ends on central escrow)
        td.capi_escrow_percentage = Decimal::new(0, 0).try_into().unwrap();
        td.specs = CreateDaoSpecs::new(
            "Pancakes ltd".to_owned(),
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore".to_owned(),
            CreateSharesSpecs {
                token_name: "PCK".to_owned(),
                supply: ShareAmount::new(300),
            },
            ShareAmount::new(300),
            FundsAmount::new(5_000_000),
            "https://placekitten.com/200/300".to_owned(),
            "https://twitter.com/capi_fin".to_owned(),
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

        // flow

        let buy_share_amount = ShareAmount::new(20);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let dividend = FundsAmount::new(200_000); // just an amount low enough so we can claim 2x

        let precs = claim_precs(
            &td,
            buy_share_amount,
            pay_and_drain_amount,
            &drainer,
            &claimer,
        )
        .await?;
        let res1 = claim_flow(&td, &precs.dao, &claimer, dividend).await?;
        let res2 = claim_flow(&td, &precs.dao, &claimer, dividend).await?;

        // test

        let total_expected_claimed_amount = res1.claimed + res2.claimed;
        test_claim_result(
            &algod,
            &claimer,
            res2.dao.central_app_id,
            td.funds_asset_id,
            res2.dao.central_escrow.address(),
            res2.dao.customer_escrow.address(),
            precs.drain_res.drained_amounts.dao,
            // 2 claims: asset balance is the total claimed amount
            res1.claimer_balance_before_claiming + total_expected_claimed_amount,
            // central lost the amount
            precs.central_escrow_balance_after_drain - total_expected_claimed_amount,
            // double check shares local state
            buy_share_amount,
            // 2 claims: local state is the total claimed amount
            total_expected_claimed_amount,
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
        central_app_id: u64,
        funds_asset_id: FundsAssetId,
        central_escrow_address: &Address,
        customer_escrow_address: &Address,
        // this parameter isn't ideal: it assumes that we did a (one) drain before claiming
        // for now letting it there as it's a quick refactoring
        // arguably needed, it tests basically that the total received global state isn't affected by claiming
        // (otherwise this is/should be already tested in the drain logic)
        drained_amount: FundsAmount,
        expected_claimer_balance: FundsAmount,
        expected_central_balance: FundsAmount,
        expected_shares: ShareAmount,
        expected_claimed_total: FundsAmount,
    ) -> Result<()> {
        let claim_funds_amount = funds_holdings(algod, &claimer.address(), funds_asset_id).await?;
        let central_escrow_funds_amount =
            funds_holdings(algod, central_escrow_address, funds_asset_id).await?;

        assert_eq!(expected_claimer_balance, claim_funds_amount);
        assert_eq!(expected_central_balance, central_escrow_funds_amount);

        // the total received didn't change
        // (i.e. same as expected after draining, claiming doesn't affect it)
        let global_state = central_global_state(algod, central_app_id).await?;
        assert_eq!(global_state.received, drained_amount);

        // sanity check: global state addresses are set
        assert_eq!(&global_state.central_escrow, central_escrow_address);
        assert_eq!(&global_state.customer_escrow, customer_escrow_address);

        // claimer local state: test that it was incremented by amount claimed
        // Only one local variable used
        let claimer_account = algod.account_information(&claimer.address()).await?;
        assert_eq!(1, claimer_account.apps_local_state.len());

        // check local state

        let investor_state = central_investor_state_from_acc(&claimer_account, central_app_id)?;

        // double-check shares count (not directly related to this test)
        assert_eq!(expected_shares, investor_state.shares);
        // check claimed total local state
        assert_eq!(expected_claimed_total, investor_state.claimed);

        Ok(())
    }
}

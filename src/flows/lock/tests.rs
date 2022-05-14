#[cfg(test)]
mod tests {
    use algonaut::transaction::{AcceptAsset, TransferAsset, TxnBuilder};
    use anyhow::Result;
    use mbase::models::{share_amount::ShareAmount, funds::FundsAmount};
    use serial_test::serial;
    use tokio::test;

    use crate::{
        algo_helpers::send_tx_and_wait,
        flows::{
            claim::claim::claimable_dividend,
            invest::app_optins::{
                invest_or_locking_app_optin_tx, submit_invest_or_locking_app_optin,
            },
        },
        network_util::wait_for_pending_transaction,
        state::{
            account_state::{
                find_asset_holding_or_err, funds_holdings, funds_holdings_from_account,
            },
            app_state::ApplicationLocalStateError,
            dao_app_state::{
                central_investor_state_from_acc, dao_global_state, dao_investor_state,
            },
            dao_shares::dao_shares,
        },
        testing::{
            flow::{
                claim_flow::claim_flow,
                create_dao_flow::create_dao_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                invest_in_dao_flow::{invests_flow, invests_optins_flow},
                lock_flow::lock_flow,
                unlock_flow::unlock_flow,
            },
            network_test_util::test_dao_init,
        },
    };

    #[test]
    #[serial]
    async fn test_lock() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.creator;

        let buy_share_amount = ShareAmount::new(10);

        // precs

        let dao = create_dao_flow(td).await?;

        invests_optins_flow(&algod, &td.investor1, &dao).await?;
        let _ = invests_flow(td, &td.investor1, buy_share_amount, &dao).await?;

        // drain (to generate dividend). note that investor doesn't reclaim it (doesn't seem relevant for this test)
        // (the draining itself may also not be relevant, just for a more realistic pre-trade scenario)
        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);
        let drain_res =
            customer_payment_and_drain_flow(td, &dao, customer_payment_amount, drainer).await?;

        // investor1 unlocks
        let traded_shares = buy_share_amount;
        let unlock_tx_id = unlock_flow(algod, &dao, &td.investor1, dao.shares_asset_id).await?;
        wait_for_pending_transaction(algod, &unlock_tx_id).await?;

        // investor2 gets shares from investor1 externally
        // normally this will be a swap in a dex. could also be a gift or some other service

        // investor2 opts in to the asset (this is done in the external service, e.g. dex)
        let params = algod.suggested_transaction_params().await?;
        let shares_optin_tx = TxnBuilder::with(
            &params,
            AcceptAsset::new(td.investor2.address(), dao.shares_asset_id).build(),
        )
        .build()?;
        let signed_shares_optin_tx = td.investor2.sign_transaction(shares_optin_tx)?;
        send_tx_and_wait(algod, &signed_shares_optin_tx).await?;

        // investor1 sends shares to investor2 (e.g. as part of atomic swap in a dex)
        let trade_tx = TxnBuilder::with(
            &params,
            TransferAsset::new(
                td.investor1.address(),
                dao.shares_asset_id,
                traded_shares.val(),
                td.investor2.address(),
            )
            .build(),
        )
        .build()?;
        let signed_trade_tx = td.investor1.sign_transaction(trade_tx)?;
        send_tx_and_wait(algod, &signed_trade_tx).await?;

        // investor2 opts in to our app. this will be on our website.
        // TODO confirm: can't we opt in in the same group (accessing local state during opt in fails)?,
        // is there a way to avoid the investor confirming txs 2 times here?

        let app_optin_tx =
            invest_or_locking_app_optin_tx(&algod, &dao, &td.investor2.address()).await?;

        let app_optin_signed_tx = td.investor2.sign_transaction(app_optin_tx)?;
        let app_optin_tx_id =
            submit_invest_or_locking_app_optin(&algod, app_optin_signed_tx).await?;
        wait_for_pending_transaction(&algod, &app_optin_tx_id).await?;

        // flow

        // investor2 locks the acquired shares
        lock_flow(algod, &dao, &td.investor2, traded_shares).await?;

        // tests

        // global state set to locked shares
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(traded_shares, gs.locked_shares);

        // investor2 lost locked assets

        let investor2_infos = algod.account_information(&td.investor2.address()).await?;
        let investor2_assets = &investor2_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor2_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor2_assets, dao.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // already claimed local state initialized to entitled funds

        // the amount drained to the central (all income so far)
        let central_total_received = drain_res.drained_amounts.dao;
        let investor2_entitled_amount = claimable_dividend(
            central_total_received,
            FundsAmount::new(0),
            dao.specs.shares.supply,
            traded_shares,
            td.precision,
            dao.specs.investors_share,
        )?;

        let investor_state = central_investor_state_from_acc(&investor2_infos, dao.app_id)?;
        // shares local state initialized to shares
        assert_eq!(traded_shares, investor_state.shares);
        // claimed total is initialized to entitled amount
        assert_eq!(investor2_entitled_amount, investor_state.claimed);

        // renaming for clarity
        let entitled_amount_after_locking_shares = investor2_entitled_amount;

        let dao_shares = dao_shares(algod, dao.app_id, dao.shares_asset_id).await?;
        // the traded shares were locked and we've no more locked shares, to we expect them in the locked global state
        assert_eq!(traded_shares, dao_shares.locked);
        // with the now "returned" shares the holdings are back to the asset total supply
        assert_eq!(dao.specs.shares_for_investors(), dao_shares.total());

        // investor2 claims: doesn't get anything, because there has not been new income (customer payments) since they bought the shares
        let _ = claim_flow(td, &dao, &td.investor2).await;

        // drain again to generate dividend and be able to claim
        let customer_payment_amount_2 = FundsAmount::new(10 * 1_000_000);
        let _ =
            customer_payment_and_drain_flow(td, &dao, customer_payment_amount_2, drainer).await?;

        // claim again: this time there's something to claim and we expect success

        // remember state
        let investor2_amount_before_claim =
            funds_holdings(algod, &td.investor2.address(), td.funds_asset_id).await?;

        let _ = claim_flow(td, &dao, &td.investor2).await?;

        // just a rename to help with clarity a bit
        let expected_claimed_amount = investor2_entitled_amount;
        println!(">>> expected_claimed_amount: {:?}", expected_claimed_amount);
        let investor2_infos = algod.account_information(&td.investor2.address()).await?;
        let investor2_amount = funds_holdings_from_account(&investor2_infos, td.funds_asset_id)?;

        // the balance is increased with the claim
        assert_eq!(
            investor2_amount_before_claim + expected_claimed_amount,
            investor2_amount
        );

        // investor's claimed local state was updated:
        let investor_state = central_investor_state_from_acc(&investor2_infos, dao.app_id)?;
        // the shares haven't changed
        assert_eq!(traded_shares, investor_state.shares);
        // the claimed total was updated:
        // initial (entitled_amount_after_locking_shares: entitled amount when locking the shares) + just claimed
        assert_eq!(
            entitled_amount_after_locking_shares + expected_claimed_amount,
            investor_state.claimed
        );

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_partial_lock() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let partial_lock_amount = ShareAmount::new(4);
        let buy_share_amount = ShareAmount::new(partial_lock_amount.val() + 6);

        // precs

        let dao = create_dao_flow(td).await?;

        invests_optins_flow(algod, investor, &dao).await?;
        let _ = invests_flow(td, investor, buy_share_amount, &dao).await?;

        // investor unlocks - note that partial unlocking isn't possible, only locking

        let unlock_tx_id = unlock_flow(algod, &dao, &investor, dao.shares_asset_id).await?;
        wait_for_pending_transaction(&algod, &unlock_tx_id).await?;

        // sanity checks

        // investor was opted out (implies: no shares locked)
        let investor_state_res = dao_investor_state(algod, &investor.address(), dao.app_id).await;
        assert_eq!(
            Err(ApplicationLocalStateError::NotOptedIn),
            investor_state_res
        );

        // investor has the unlocks shares

        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.shares_asset_id)?;
        assert_eq!(buy_share_amount.0, shares_asset.amount);

        // investor locks again a part of the shares

        // optin to app
        let app_optins_tx =
            invest_or_locking_app_optin_tx(algod, &dao, &investor.address()).await?;
        let app_optin_signed_tx = investor.sign_transaction(app_optins_tx)?;
        let app_optin_tx_id =
            submit_invest_or_locking_app_optin(algod, app_optin_signed_tx).await?;
        wait_for_pending_transaction(algod, &app_optin_tx_id).await?;

        // lock
        lock_flow(algod, &dao, investor, partial_lock_amount).await?;

        // tests

        // global state set to locked shares
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(partial_lock_amount, gs.locked_shares);

        // investor locked the shares
        let investor_state = dao_investor_state(&algod, &investor.address(), dao.app_id).await?;
        assert_eq!(partial_lock_amount, investor_state.shares);

        // investor has the remaining free shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.shares_asset_id)?;
        assert_eq!(
            buy_share_amount.val() - partial_lock_amount.val(),
            shares_asset.amount
        );

        Ok(())
    }
}

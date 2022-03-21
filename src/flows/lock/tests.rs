#[cfg(test)]
mod tests {
    use algonaut::transaction::{AcceptAsset, TransferAsset, TxnBuilder};
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        algo_helpers::send_tx_and_wait,
        flows::{
            claim::claim::claimable_dividend,
            create_dao::share_amount::ShareAmount,
            invest::app_optins::{
                invest_or_locking_app_optin_tx, submit_invest_or_locking_app_optin,
            },
        },
        funds::FundsAmount,
        network_util::wait_for_pending_transaction,
        state::{
            account_state::{
                find_asset_holding_or_err, funds_holdings, funds_holdings_from_account,
            },
            app_state::ApplicationLocalStateError,
            central_app_state::{central_investor_state, central_investor_state_from_acc},
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

        invests_optins_flow(&algod, &td.investor1, &dao.dao).await?;
        let _ = invests_flow(td, &td.investor1, buy_share_amount, &dao.dao, &dao.dao_id).await?;

        // drain (to generate dividend). note that investor doesn't reclaim it (doesn't seem relevant for this test)
        // (the draining itself may also not be relevant, just for a more realistic pre-trade scenario)
        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);
        let drain_res =
            customer_payment_and_drain_flow(td, &dao.dao, customer_payment_amount, drainer).await?;

        // investor1 unlocks
        let traded_shares = buy_share_amount;
        let unlock_tx_id = unlock_flow(algod, &dao.dao, &td.investor1, traded_shares).await?;
        wait_for_pending_transaction(algod, &unlock_tx_id).await?;

        // investor2 gets shares from investor1 externally
        // normally this will be a swap in a dex. could also be a gift or some other service

        // investor2 opts in to the asset (this is done in the external service, e.g. dex)
        let params = algod.suggested_transaction_params().await?;
        let shares_optin_tx = TxnBuilder::with(
            &params,
            AcceptAsset::new(td.investor2.address(), dao.dao.shares_asset_id).build(),
        )
        .build()?;
        let signed_shares_optin_tx = td.investor2.sign_transaction(shares_optin_tx)?;
        send_tx_and_wait(algod, &signed_shares_optin_tx).await?;

        // investor1 sends shares to investor2 (e.g. as part of atomic swap in a dex)
        let trade_tx = TxnBuilder::with(
            &params,
            TransferAsset::new(
                td.investor1.address(),
                dao.dao.shares_asset_id,
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
            invest_or_locking_app_optin_tx(&algod, &dao.dao, &td.investor2.address()).await?;

        let app_optin_signed_tx = td.investor2.sign_transaction(app_optin_tx)?;
        let app_optin_tx_id =
            submit_invest_or_locking_app_optin(&algod, app_optin_signed_tx).await?;
        wait_for_pending_transaction(&algod, &app_optin_tx_id).await?;

        // flow

        // investor2 locks the acquired shares
        lock_flow(algod, &dao.dao, &dao.dao_id, &td.investor2, traded_shares).await?;

        // tests

        // investor2 lost locked assets

        let investor2_infos = algod.account_information(&td.investor2.address()).await?;
        let investor2_assets = &investor2_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor2_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor2_assets, dao.dao.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // already claimed local state initialized to entitled funds

        // the amount drained to the central (all income so far)
        let central_total_received = drain_res.drained_amounts.dao;
        let investor2_entitled_amount = claimable_dividend(
            central_total_received,
            FundsAmount::new(0),
            dao.dao.specs.shares.supply,
            traded_shares,
            td.precision,
            dao.dao.specs.investors_part(),
        )?;

        let investor_state =
            central_investor_state_from_acc(&investor2_infos, dao.dao.central_app_id)?;
        // shares local state initialized to shares
        assert_eq!(traded_shares, investor_state.shares);
        // claimed total is initialized to entitled amount
        assert_eq!(investor2_entitled_amount, investor_state.claimed);

        // renaming for clarity
        let total_withdrawn_after_locking_setup_call = investor2_entitled_amount;

        // locking escrow got assets
        let locking_escrow_infos = algod
            .account_information(dao.dao.locking_escrow.address())
            .await?;
        let locking_escrow_assets = locking_escrow_infos.assets;
        assert_eq!(1, locking_escrow_assets.len()); // opted in to shares
        assert_eq!(traded_shares.0, locking_escrow_assets[0].amount);

        // investor2 claims: doesn't get anything, because there has not been new income (customer payments) since they bought the shares
        // the dividend is the smallest number possible, to show that we can't retrieve anything
        let claim_flow_res = claim_flow(td, &dao.dao, &td.investor2, FundsAmount::new(1)).await;
        log::debug!("Expected error claiming: {:?}", claim_flow_res);
        // If there's nothing to claim, the smart contract fails (transfer amount > allowed)
        assert!(claim_flow_res.is_err());

        // drain again to generate dividend and be able to claim
        let customer_payment_amount_2 = FundsAmount::new(10 * 1_000_000);
        let drain_res2 =
            customer_payment_and_drain_flow(td, &dao.dao, customer_payment_amount_2, drainer)
                .await?;

        // claim again: this time there's something to claim and we expect success

        // remember state
        let investor2_amount_before_claim =
            funds_holdings(algod, &td.investor2.address(), td.funds_asset_id).await?;

        // we'll claim the max possible amount
        let investor2_entitled_amount = claimable_dividend(
            drain_res2.drained_amounts.dao,
            FundsAmount::new(0),
            dao.dao.specs.shares.supply,
            traded_shares,
            td.precision,
            dao.dao.specs.investors_part(),
        )?;
        log::debug!(
            "Claiming max possible amount (expected to succeed): {:?}",
            investor2_entitled_amount
        );
        let _ = claim_flow(td, &dao.dao, &td.investor2, investor2_entitled_amount).await?;

        // just a rename to help with clarity a bit
        let expected_claimed_amount = investor2_entitled_amount;
        let investor2_infos = algod.account_information(&td.investor2.address()).await?;
        let investor2_amount = funds_holdings_from_account(&investor2_infos, td.funds_asset_id)?;

        // the balance is increased with the claim
        assert_eq!(
            investor2_amount_before_claim + expected_claimed_amount,
            investor2_amount
        );

        // investor's claimed local state was updated:
        let investor_state =
            central_investor_state_from_acc(&investor2_infos, dao.dao.central_app_id)?;
        // the shares haven't changed
        assert_eq!(traded_shares, investor_state.shares);
        // the claimed total was updated:
        // initial (total_withdrawn_after_locking_setup_call: entitled amount when locking the shares) + just claimed
        assert_eq!(
            total_withdrawn_after_locking_setup_call + expected_claimed_amount,
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

        invests_optins_flow(algod, investor, &dao.dao).await?;
        let _ = invests_flow(td, investor, buy_share_amount, &dao.dao, &dao.dao_id).await?;

        // investor unlocks - note that partial unlocking isn't possible, only locking

        let unlock_tx_id = unlock_flow(algod, &dao.dao, &investor, buy_share_amount).await?;
        wait_for_pending_transaction(&algod, &unlock_tx_id).await?;

        // sanity checks

        // investor was opted out (implies: no shares locked)
        let investor_state_res =
            central_investor_state(algod, &investor.address(), dao.dao.central_app_id).await;
        assert_eq!(
            Err(ApplicationLocalStateError::NotOptedIn),
            investor_state_res
        );

        // investor has the unlocks shares

        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.dao.shares_asset_id)?;
        assert_eq!(buy_share_amount.0, shares_asset.amount);

        // investor locks again a part of the shares

        // optin to app
        let app_optins_tx =
            invest_or_locking_app_optin_tx(algod, &dao.dao, &investor.address()).await?;
        let app_optin_signed_tx = investor.sign_transaction(app_optins_tx)?;
        let app_optin_tx_id =
            submit_invest_or_locking_app_optin(algod, app_optin_signed_tx).await?;
        wait_for_pending_transaction(algod, &app_optin_tx_id).await?;

        // lock
        lock_flow(algod, &dao.dao, &dao.dao_id, investor, partial_lock_amount).await?;

        // tests

        // investor locked the shares
        let investor_state =
            central_investor_state(&algod, &investor.address(), dao.dao.central_app_id).await?;
        assert_eq!(partial_lock_amount, investor_state.shares);

        // investor has the remaining free shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.dao.shares_asset_id)?;
        assert_eq!(
            buy_share_amount.val() - partial_lock_amount.val(),
            shares_asset.amount
        );

        Ok(())
    }
}

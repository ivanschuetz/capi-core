#[cfg(test)]
mod tests {
    use algonaut::{algod::v2::Algod, core::to_app_address, transaction::account::Account};
    use anyhow::Result;
    use mbase::{
        models::{funds::FundsAmount, share_amount::ShareAmount},
        state::dao_app_state::{central_investor_state_from_acc, dao_global_state},
    };
    use serial_test::serial;
    use tokio::test;

    use crate::{
        flows::create_dao::model::Dao,
        network_util::wait_for_pending_transaction,
        state::{account_state::find_asset_holding_or_err, dao_shares::dao_shares},
        testing::{
            flow::{
                create_dao_flow::create_dao_flow,
                invest_in_dao_flow::{invests_flow, invests_optins_flow},
                unlock_flow::unlock_flow,
            },
            network_test_util::test_dao_init,
        },
    };

    #[test]
    #[serial]
    async fn test_unlock() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);

        // precs

        let dao = create_dao_flow(td).await?;

        invests_optins_flow(&algod, &investor, &dao).await?;
        let _ = invests_flow(td, investor, buy_share_amount, &dao).await?;
        // TODO double check tests for state (at least important) tested (e.g. investor has shares, locking doesn't etc.)

        pre_unlock_flow_sanity_tests(algod, investor, &dao, buy_share_amount).await?;

        // flow

        let unlock_tx_id = unlock_flow(algod, &dao, investor).await?;
        wait_for_pending_transaction(algod, &unlock_tx_id).await?;

        // test

        // global state decremented
        let gs = dao_global_state(algod, dao.app_id).await?;
        // complete unlock and no one else has locked shares - we expect 0 locked shares in global state
        assert_eq!(ShareAmount::new(0), gs.locked_shares);

        // shares not anymore in app escrow
        let app_escrow_infos = algod.account_information(&dao.app_address()).await?;
        let app_escrow_assets = app_escrow_infos.assets;
        assert_eq!(3, app_escrow_assets.len()); // still opted in to shares (and funds asset and image nft)
        let dao_shares = dao_shares(algod, dao.app_id, dao.shares_asset_id).await?;
        assert_eq!(ShareAmount::new(0), dao_shares.locked); // lost shares

        // investor got shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.shares_asset_id)?;
        // got the shares
        assert_eq!(buy_share_amount.0, shares_asset.amount);

        // investor local state cleared (opted out)
        assert_eq!(0, investor_infos.apps_local_state.len());

        Ok(())
    }

    async fn pre_unlock_flow_sanity_tests(
        algod: &Algod,
        investor: &Account,
        dao: &Dao,
        buy_share_amount: ShareAmount,
    ) -> Result<()> {
        // double check investor's assets

        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset = find_asset_holding_or_err(&investor_assets, dao.shares_asset_id)?;
        // doesn't have shares (they're sent directly to app escrow)
        assert_eq!(0, shares_asset.amount);

        let investor_state = central_investor_state_from_acc(&investor_infos, dao.app_id)?;
        // double check investor's local state
        // shares set to bought asset amount
        assert_eq!(buy_share_amount, investor_state.shares);
        //  claimed total is 0 (hasn't claimed yet)
        assert_eq!(FundsAmount::new(0), investor_state.claimed);

        // double check app's assets
        let app_escrow_infos = algod
            .account_information(&to_app_address(dao.app_id.0))
            .await?;
        let app_escrow_assets = app_escrow_infos.assets;
        assert_eq!(2, app_escrow_assets.len()); // opted in to shares, funds asset

        let dao_shares = dao_shares(algod, dao.app_id, dao.shares_asset_id).await?;
        assert_eq!(buy_share_amount, dao_shares.locked);

        // check locked global state (assumes only share amount has been locked)
        let gs = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(buy_share_amount, gs.locked_shares);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        flows::create_dao::model::Dao,
        state::{
            account_state::{asset_holdings, find_asset_holding_or_err},
            app_state::ApplicationLocalStateError,
            dao_app_state::{dao_global_state, dao_investor_state},
            dao_shares::dao_shares,
        },
        testing::{
            flow::create_dao_flow::create_dao_flow,
            network_test_util::{test_dao_init, TestDeps},
        },
    };
    use algonaut::algod::v2::Algod;
    use anyhow::Result;
    use mbase::models::{asset_amount::AssetAmount, funds::FundsAmount, share_amount::ShareAmount};
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_create_dao_flow() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        let dao = create_dao_flow(td).await?;

        log::debug!("Submitted create dao txs, dao: {:?}", dao);

        let creator_infos = algod.account_information(&td.creator.address()).await?;
        let created_assets = creator_infos.created_assets;

        assert_eq!(created_assets.len(), 1);

        log::debug!("created_assets {:?}", created_assets);

        // created asset checks
        // assert_eq!(created_assets[0].params.creator, td.creator.address()); // TODO clarify creator field
        // name matches specs
        assert_eq!(
            created_assets[0].params.name,
            Some(dao.specs.shares.token_name.clone())
        );
        // unit matches specs
        assert_eq!(
            created_assets[0].params.unit_name,
            Some(dao.specs.shares.token_name.clone())
        );
        assert_eq!(td.specs.shares.supply.0, created_assets[0].params.total);
        let creator_assets = creator_infos.assets;
        // funds asset + not opted-out from shares (TODO maybe do this, no reason for creator to be opted in in the investor assets) so still there
        assert_eq!(2, creator_assets.len());
        // creator sent all the investor shares to the escrow (during dao creation): has the remaining ones
        let shares_asset = find_asset_holding_or_err(&creator_assets, dao.shares_asset_id)?;
        assert_eq!(
            td.specs.shares_for_creator(),
            ShareAmount::new(shares_asset.amount)
        );

        // app escrow funding checks
        let app_escrow = &dao.app_address();
        let app_escrow_infos = algod.account_information(app_escrow).await?;
        let app_escrow_held_assets = app_escrow_infos.assets;
        assert_eq!(app_escrow_held_assets.len(), 2);
        let app_escrow_shares =
            ShareAmount(asset_holdings(algod, app_escrow, dao.shares_asset_id).await?);
        let app_funds = asset_holdings(algod, app_escrow, dao.funds_asset_id.0).await?;
        assert_eq!(td.specs.shares_for_investors(), app_escrow_shares); // the app's escrow has the share supply for investors
        assert_eq!(AssetAmount(0), app_funds); // no funds yet

        let dao_shares = dao_shares(algod, dao.app_id, dao.shares_asset_id).await?;
        assert_eq!(ShareAmount::new(0), dao_shares.locked); // there are no locked shares
        assert_eq!(td.specs.shares_for_investors(), dao_shares.available); // all the shares on the app's escrow are available (not locked)

        test_global_app_state_setup_correctly(algod, &dao, td).await?;

        // sanity check: the creator doesn't opt in to the app (doesn't invest or lock)
        let central_investor_state_res =
            dao_investor_state(&algod, &td.creator.address(), dao.app_id).await;
        assert_eq!(
            Err(ApplicationLocalStateError::NotOptedIn),
            central_investor_state_res
        );

        Ok(())
    }

    async fn test_global_app_state_setup_correctly(
        algod: &Algod,
        dao: &Dao,
        td: &TestDeps,
    ) -> Result<()> {
        let state = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(
            dao.customer_escrow.to_versioned_address(),
            state.customer_escrow
        );
        assert_eq!(td.funds_asset_id, state.funds_asset_id);
        assert_eq!(dao.shares_asset_id, state.shares_asset_id);
        assert_eq!(FundsAmount::new(0), state.received);
        Ok(())
    }
}

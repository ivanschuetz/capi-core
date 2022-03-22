#[cfg(test)]
mod tests {
    use crate::{
        flows::create_dao::model::Dao,
        funds::FundsAmount,
        state::{
            account_state::find_asset_holding_or_err,
            app_state::ApplicationLocalStateError,
            central_app_state::{dao_global_state, dao_investor_state},
        },
        testing::{
            flow::create_dao_flow::create_dao_flow,
            network_test_util::{test_dao_init, TestDeps},
        },
    };
    use algonaut::algod::v2::Algod;
    use anyhow::Result;
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
        assert_eq!(created_assets[0].params.creator, td.creator.address());
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
        // creator sent all the shares to the escrow (during dao creation): has 0
        let shares_asset = find_asset_holding_or_err(&creator_assets, dao.shares_asset_id)?;
        assert_eq!(0, shares_asset.amount);

        // investing escrow funding checks
        let escrow = &dao.invest_escrow;
        let escrow_infos = algod.account_information(escrow.address()).await?;
        // TODO refactor and check min algos balance
        let escrow_held_assets = escrow_infos.assets;
        assert_eq!(escrow_held_assets.len(), 1);
        assert_eq!(escrow_held_assets[0].asset_id, dao.shares_asset_id);
        assert_eq!(escrow_held_assets[0].amount, dao.specs.shares.supply.val());

        // locking escrow funding checks
        let locking_escrow = &dao.locking_escrow;
        let locking_escrow_infos = algod.account_information(locking_escrow.address()).await?;
        let locking_escrow_held_assets = locking_escrow_infos.assets;
        // TODO refactor and check min algos balance
        assert_eq!(locking_escrow_held_assets.len(), 1);
        assert_eq!(locking_escrow_held_assets[0].asset_id, dao.shares_asset_id);
        assert_eq!(locking_escrow_held_assets[0].amount, 0); // nothing locked yet

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
        assert_eq!(dao.central_escrow.address(), &state.central_escrow);
        assert_eq!(dao.customer_escrow.address(), &state.customer_escrow);
        assert_eq!(td.funds_asset_id, state.funds_asset_id);
        assert_eq!(dao.shares_asset_id, state.shares_asset_id);
        assert_eq!(FundsAmount::new(0), state.received);
        Ok(())
    }
}

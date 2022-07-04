#[cfg(test)]
mod tests {
    use crate::{
        flows::update_data::update_data::UpdatableDaoData,
        testing::{
            flow::{
                claim_flow::{claim_flow, test::claim_precs_with_dao},
                create_dao_flow::create_dao_flow,
                update_dao_data_flow::update_dao_data_flow,
            },
            network_test_util::test_dao_init,
        },
    };
    use anyhow::Result;
    use mbase::{
        models::{funds::FundsAmount, hash::GlobalStateHash, share_amount::ShareAmount},
        state::dao_app_state::{dao_global_state, dao_investor_state, CentralAppGlobalState},
    };
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_update_data_works() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        let owner = &td.creator;
        let dao = create_dao_flow(td).await?;

        let update_data = some_data_to_update();

        // precs

        // sanity check: current state is different to the new one
        let gs_before_update = dao_global_state(algod, dao.app_id).await?;
        sanity_check_current_state_different_to_update_data(&gs_before_update, &update_data);

        // flow

        update_dao_data_flow(td, &dao, &owner, &update_data).await?;

        // test

        let gs_after_update = dao_global_state(algod, dao.app_id).await?;
        validate_global_state_with_update_data(&gs_after_update, &update_data);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_update_data_does_not_affect_other_state() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        let owner = &td.creator;
        let dao = create_dao_flow(td).await?;

        let update_data = some_data_to_update();

        // precs

        // invest and claim some dividend - after this all the app's global and local variables should be set to something
        let investor = &td.investor2;
        let drainer = &td.investor1;
        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let precs = claim_precs_with_dao(
            &td,
            &dao,
            buy_share_amount,
            pay_and_drain_amount,
            drainer,
            investor,
        )
        .await?;
        claim_flow(&td, &precs.dao, investor).await?;

        let gs_before_update = dao_global_state(&td.algod, dao.app_id).await?;
        let ls_before_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        // sanity check: current state is different to the new one
        sanity_check_current_state_different_to_update_data(&gs_before_update, &update_data);

        // flow

        update_dao_data_flow(td, &dao, &owner, &update_data).await?;

        // test

        let gs_after_update = dao_global_state(algod, dao.app_id).await?;
        let ls_after_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        validate_global_state_with_update_data(&gs_after_update, &update_data);

        // aside of what we updated, data.global state stays the same
        assert_eq!(
            gs_after_update.funds_asset_id,
            gs_before_update.funds_asset_id
        );
        assert_eq!(gs_after_update.received, gs_before_update.received);
        assert_eq!(
            gs_after_update.shares_asset_id,
            gs_before_update.shares_asset_id
        );

        // local state stays the same
        assert_eq!(ls_before_update, ls_after_update);

        Ok(())
    }

    fn some_data_to_update() -> UpdatableDaoData {
        // arbitrary data different to the existing one
        let new_project_name = "new_project_name".to_owned();
        let new_project_desc = Some(GlobalStateHash("new_project_desc".to_owned()));
        let new_image_hash = Some(GlobalStateHash("new_test_image_hash".to_owned()));
        let new_image_url = Some("new_image_url".to_owned());
        let new_social_media_url = "new_social_media_url".to_owned();

        UpdatableDaoData {
            project_name: new_project_name.clone(),
            project_desc: new_project_desc.clone(),
            image_hash: new_image_hash,
            image_url: new_image_url,
            social_media_url: new_social_media_url.clone(),
        }
    }

    fn validate_global_state_with_update_data(gs: &CentralAppGlobalState, data: &UpdatableDaoData) {
        assert_eq!(gs.project_name, data.project_name);
        assert_eq!(gs.project_desc, data.project_desc);
        assert_eq!(gs.image_hash, data.image_hash);

        if gs.image_nft.is_some() {
            let nft = gs.image_nft.clone().unwrap();
            assert!(data.image_url.is_some());
            assert_eq!(nft.url, data.image_url.clone().unwrap());
        } else {
            assert!(data.image_url.is_none())
        }

        assert_eq!(gs.social_media_url, data.social_media_url);
    }

    fn sanity_check_current_state_different_to_update_data(
        gs: &CentralAppGlobalState,
        data: &UpdatableDaoData,
    ) {
        assert_ne!(gs.project_name, data.project_name);
        assert_ne!(gs.project_desc, data.project_desc);
        assert_ne!(gs.image_hash, data.image_hash);
        assert_ne!(gs.social_media_url, data.social_media_url);
    }
}

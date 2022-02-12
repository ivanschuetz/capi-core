#[cfg(test)]
mod tests {
    use crate::{
        dependencies,
        state::central_app_state::central_investor_state,
        testing::{
            flow::create_project_flow::create_project_flow, test_data::project_specs,
            TESTS_DEFAULT_PRECISION,
        },
        testing::{network_test_util::test_init, test_data::creator},
    };
    use algonaut::core::MicroAlgos;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_create_project_flow() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();

        // UI
        let specs = project_specs();

        let precision = TESTS_DEFAULT_PRECISION;
        let project = create_project_flow(&algod, &creator, &specs, precision).await?;

        // UI
        log::debug!("Submitted create project txs, project: {:?}", project);

        let creator_infos = algod.account_information(&creator.address()).await?;
        let created_assets = creator_infos.created_assets;

        assert_eq!(created_assets.len(), 1);

        log::debug!("created_assets {:?}", created_assets);

        // created asset checks
        assert_eq!(created_assets[0].params.creator, creator.address());
        // name matches specs
        assert_eq!(
            created_assets[0].params.name,
            Some(project.project.specs.shares.token_name.clone())
        );
        // unit matches specs
        assert_eq!(
            created_assets[0].params.unit_name,
            Some(project.project.specs.shares.token_name.clone())
        );
        assert_eq!(specs.shares.count, created_assets[0].params.total);
        let creator_assets = creator_infos.assets;
        // creator sent the investor's assets to the escrow and staked theirs: has 0 shares
        assert_eq!(1, creator_assets.len()); // not opted-out (TODO maybe do this, no reason for creator to be opted in in the investor assets) so still there
        assert_eq!(0, creator_assets[0].amount);

        // investing escrow funding checks
        let escrow = project.project.invest_escrow;
        let escrow_infos = algod.account_information(escrow.address()).await?;
        // TODO refactor and check min algos balance
        let escrow_held_assets = escrow_infos.assets;
        assert_eq!(escrow_held_assets.len(), 1);
        assert_eq!(
            project.project.shares_asset_id,
            escrow_held_assets[0].asset_id,
        );
        assert_eq!(
            project.project.specs.investors_part(),
            escrow_held_assets[0].amount,
        );

        // staking escrow funding checks
        let staking_escrow = project.project.staking_escrow;
        let staking_escrow_infos = algod.account_information(staking_escrow.address()).await?;
        let staking_escrow_held_assets = staking_escrow_infos.assets;
        // TODO refactor and check min algos balance
        assert_eq!(staking_escrow_held_assets.len(), 1);
        assert_eq!(
            staking_escrow_held_assets[0].asset_id,
            project.project.shares_asset_id
        );
        // the creator's shares are in the staking escrow
        assert_eq!(specs.creator_part(), staking_escrow_held_assets[0].amount);

        // the creator's central app local state is initialized correctly

        let central_state =
            central_investor_state(&algod, &creator.address(), project.project.central_app_id)
                .await?;
        // the staked shares
        assert_eq!(project.project.specs.creator_part(), central_state.shares);
        // nothing has been harvested yet
        assert_eq!(MicroAlgos(0), central_state.harvested);
        // project id initialized
        assert_eq!(project.project_id, central_state.project_id);

        Ok(())
    }
}

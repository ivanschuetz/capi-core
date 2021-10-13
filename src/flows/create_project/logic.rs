use algonaut::{algod::v2::Algod, core::Address, transaction::tx_group::TxGroup};
use anyhow::{anyhow, Result};

use crate::{
    flows::create_project::{
        model::Project,
        setup::{
            create_app::create_app_tx, create_withdrawal_slots::create_withdrawal_slots_txs,
            drain::setup_drain, investing_escrow::setup_investing_escrow_txs,
            staking_escrow::setup_staking_escrow_txs, votein_escrow::setup_votein_escrow_txs,
            votes_out_escrow::create_votes_out_escrow_tx,
        },
    },
    network_util::wait_for_pending_transaction,
    teal::{TealSource, TealSourceTemplate},
};

use super::model::{
    CreateProjectSigned, CreateProjectSpecs, CreateProjectToSign, SubmitCreateProjectResult,
};

pub async fn create_project_txs(
    algod: &Algod,
    specs: &CreateProjectSpecs,
    creator: Address,
    shares_asset_id: u64,
    votes_asset_id: u64,
    programs: Programs,
) -> Result<CreateProjectToSign> {
    log::debug!(
        "Creating project: {:?}, shares_asset_id: {}, votes_asset_id: {}",
        specs.name,
        shares_asset_id,
        votes_asset_id
    );

    // TODO reuse transaction params for all these txs, also in other places

    let mut drain_to_sign = setup_drain(
        algod,
        programs.central_escrow,
        programs.customer_escrow,
        &creator,
    )
    .await?;
    let create_app_tx = create_app_tx(
        algod,
        programs.central_app_approval,
        programs.central_app_clear,
        &creator,
        shares_asset_id,
        specs.shares.count,
    )
    .await?;
    // let mut create_app_tx = create_app_tx(algod, &creator).await?;

    // TODO why do we do this (invest and staking escrows setup) here instead of directly on project creation? there seem to be no deps on post-creation things?
    let mut setup_staking_escrow_to_sign = setup_staking_escrow_txs(
        algod,
        programs.staking_escrow,
        shares_asset_id,
        votes_asset_id,
        specs.shares.count,
        &creator,
    )
    .await?;
    let mut setup_invest_escrow_to_sign = setup_investing_escrow_txs(
        algod,
        programs.invest_escrow,
        shares_asset_id,
        votes_asset_id,
        specs.shares.count,
        specs.asset_price,
        &creator,
        setup_staking_escrow_to_sign.escrow.address,
    )
    .await?;

    let mut vote_out_to_sign = create_votes_out_escrow_tx(
        algod,
        programs.vote_out_escrow,
        creator,
        shares_asset_id,
        votes_asset_id,
        setup_staking_escrow_to_sign.escrow.address,
    )
    .await?;
    let mut vote_in_to_sign = setup_votein_escrow_txs(
        algod,
        programs.vote_in_escrow,
        creator,
        votes_asset_id,
        specs.vote_threshold_units(),
        vote_out_to_sign.escrow.address,
    )
    .await?;

    //////////////////////////////
    // withdrawal slots
    // TODO clarify whether we need asset id here, otherwise move signing to first group with asset creation.
    // we need the slot app ids in the central app (this validation is not implemented yet), so doing it here is temporary.
    let create_withdrawal_slots_txs = create_withdrawal_slots_txs(
        algod,
        3,
        programs.withdrawal_slot_approval,
        programs.withdrawal_slot_clear,
        &creator,
        specs.vote_threshold,
    )
    .await?;

    //////////////////////////////
    // asset opt-ins (have to be before the other transactions)
    TxGroup::assign_group_id(vec![
        &mut setup_staking_escrow_to_sign.escrow_shares_optin_tx,
        &mut setup_staking_escrow_to_sign.escrow_votes_optin_tx,
        &mut setup_invest_escrow_to_sign.escrow_shares_optin_tx,
        &mut setup_invest_escrow_to_sign.escrow_votes_optin_tx,
        &mut vote_out_to_sign.escrow_votes_optin_tx,
        &mut vote_in_to_sign.escrow_votes_optin_tx,
    ])?;

    // Now that the lsig txs have been assigned a group id, sign (by their respective programs)
    let staking_escrow = setup_staking_escrow_to_sign.escrow.clone();
    let staking_escrow_shares_optin_tx_signed =
        staking_escrow.sign(&setup_staking_escrow_to_sign.escrow_shares_optin_tx, vec![])?;
    let staking_escrow_votes_optin_tx_signed =
        staking_escrow.sign(&setup_staking_escrow_to_sign.escrow_votes_optin_tx, vec![])?;
    let invest_escrow = setup_invest_escrow_to_sign.escrow.clone();
    let invest_escrow_shares_optin_tx_signed =
        invest_escrow.sign(&setup_invest_escrow_to_sign.escrow_shares_optin_tx, vec![])?;
    let invest_escrow_votes_optin_tx_signed =
        invest_escrow.sign(&setup_invest_escrow_to_sign.escrow_votes_optin_tx, vec![])?;
    let votes_out_escrow_votes_optin_tx_signed = vote_out_to_sign
        .escrow
        .sign(&vote_out_to_sign.escrow_votes_optin_tx, vec![])?;
    let votes_in_escrow_votes_optin_tx_signed = vote_in_to_sign
        .escrow
        .sign(&vote_in_to_sign.escrow_votes_optin_tx, vec![])?;

    let optin_txs = vec![
        staking_escrow_shares_optin_tx_signed,
        staking_escrow_votes_optin_tx_signed,
        invest_escrow_shares_optin_tx_signed,
        invest_escrow_votes_optin_tx_signed,
        votes_out_escrow_votes_optin_tx_signed,
        votes_in_escrow_votes_optin_tx_signed,
    ];

    //////////////////////////////

    TxGroup::assign_group_id(vec![
        &mut drain_to_sign.central.fund_min_balance_tx,
        &mut drain_to_sign.customer.fund_min_balance_tx,
        &mut vote_out_to_sign.escrow_funding_algos_tx,
        &mut vote_in_to_sign.escrow_funding_algos_tx,
        &mut setup_staking_escrow_to_sign.escrow_funding_algos_tx,
        &mut setup_invest_escrow_to_sign.escrow_funding_algos_tx,
    ])?;

    Ok(CreateProjectToSign {
        specs: specs.to_owned(),
        creator,

        staking_escrow: setup_staking_escrow_to_sign.escrow,
        invest_escrow: setup_invest_escrow_to_sign.escrow,
        central_escrow: drain_to_sign.central.escrow,
        customer_escrow: drain_to_sign.customer.escrow,
        vote_out_escrow: vote_out_to_sign.escrow,
        votein_escrow: vote_in_to_sign.escrow,

        // initial funding (algos) round, to be signed by creator
        escrow_funding_txs: vec![
            drain_to_sign.central.fund_min_balance_tx,
            drain_to_sign.customer.fund_min_balance_tx,
            vote_out_to_sign.escrow_funding_algos_tx,
            vote_in_to_sign.escrow_funding_algos_tx,
            setup_staking_escrow_to_sign.escrow_funding_algos_tx,
            setup_invest_escrow_to_sign.escrow_funding_algos_tx,
        ],
        optin_txs,
        create_app_tx,
        create_withdrawal_slots_txs,

        // xfers to escrows: have to be executed after escrows are opted in
        xfer_shares_to_invest_escrow: setup_invest_escrow_to_sign.escrow_funding_shares_asset_tx,
        xfer_votes_to_invest_escrow: setup_invest_escrow_to_sign.escrow_funding_votes_asset_tx,
    })
}

pub async fn submit_create_project(
    algod: &Algod,
    signed: CreateProjectSigned,
) -> Result<SubmitCreateProjectResult> {
    log::debug!(
        "Submitting created project specs: {:?}, creator: {:?}",
        signed.specs,
        signed.creator
    );

    log::debug!(
        "broadcasting project creation transactions({:?})",
        signed.escrow_funding_txs.len()
    );

    // crate::teal::debug_teal_rendered(&signed.optin_txs, "investing_escrow").unwrap();

    let _ = algod
        .broadcast_signed_transactions(&signed.escrow_funding_txs)
        .await?;

    ///////////////////////////////////////////////////////////¯
    ///////////////////////////////////////////////////////////¯
    // Create withdrawal slot apps
    log::debug!("Creating withdrawal slot apps..");
    let mut withdrawal_slot_app_ids = vec![];
    for withdrawal_slot_app in &signed.create_withdrawal_slots_txs {
        let create_app_res = algod
            .broadcast_signed_transaction(&withdrawal_slot_app)
            .await?;
        let p_tx = wait_for_pending_transaction(algod, &create_app_res.tx_id)
            .await?
            .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
        let app_id = p_tx
            .application_index
            .ok_or_else(|| anyhow!("Pending tx didn't have app id"))?;
        log::debug!("Created withdrawal slot app id: {}", app_id);
        withdrawal_slot_app_ids.push(app_id);
    }
    // Not really necessary (we exit if any of the requests creating the slot apps fails), just triple-check
    if withdrawal_slot_app_ids.len() != signed.create_withdrawal_slots_txs.len() {
        return Err(anyhow!("Couldn't create apps for all withdrawal slots"));
    }

    ///////////////////////////////////////////////////////////¯
    // TODO investigate: application_index is None in p_tx when executing the app create tx together with the other txs
    // see more notes in old repo
    ///////////////////////////////////////////////////////////¯
    log::debug!("Creating central app..");
    // let central_app_id = p_tx
    //     .application_index
    //     .ok_or(anyhow!("Pending tx didn't have app id"))?;
    let create_app_res = algod
        .broadcast_signed_transaction(&signed.create_app_tx)
        .await?;
    let p_tx = wait_for_pending_transaction(algod, &create_app_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    let central_app_id = p_tx
        .application_index
        .ok_or_else(|| anyhow!("Pending tx didn't have app id"))?;
    log::debug!("?? (see todo) central_app_id: {:?}", central_app_id);

    ///////////////////////////////////////////////////////////¯
    ///////////////////////////////////////////////////////////¯
    // Now that the escrows are funded, opt them in

    log::debug!(
        "broadcasting project creation opt ins({:?})",
        signed.optin_txs.len()
    );
    let submit_grouped_optin_txs_res = algod
        .broadcast_signed_transactions(&signed.optin_txs)
        .await?;
    let _ = wait_for_pending_transaction(algod, &submit_grouped_optin_txs_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    log::debug!("Executed optin txs");

    // now that the escrows are opted in, send them assets
    let submit_shares_xfer_tx_res = algod
        .broadcast_signed_transaction(&signed.xfer_shares_to_invest_escrow)
        .await?;
    let submit_votes_xfer_tx_res = algod
        .broadcast_signed_transaction(&signed.xfer_votes_to_invest_escrow)
        .await?;
    let _ = wait_for_pending_transaction(algod, &submit_shares_xfer_tx_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    let _ = wait_for_pending_transaction(algod, &submit_votes_xfer_tx_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    log::debug!("Executed escrow xfer txs");

    ///////////

    Ok(SubmitCreateProjectResult {
        project: Project {
            specs: signed.specs,
            shares_asset_id: signed.shares_asset_id,
            votes_asset_id: signed.votes_asset_id,
            central_app_id,
            invest_escrow: signed.invest_escrow,
            staking_escrow: signed.staking_escrow,
            customer_escrow: signed.customer_escrow,
            central_escrow: signed.central_escrow,
            vote_out_escrow: signed.vote_out_escrow,
            votein_escrow: signed.votein_escrow,
            creator: signed.creator,
        },
    })
}

pub struct Programs {
    pub central_app_approval: TealSourceTemplate,
    pub central_app_clear: TealSource,
    pub withdrawal_slot_approval: TealSourceTemplate,
    pub withdrawal_slot_clear: TealSource,
    pub central_escrow: TealSourceTemplate,
    pub customer_escrow: TealSourceTemplate,
    pub invest_escrow: TealSourceTemplate,
    pub staking_escrow: TealSourceTemplate,
    pub vote_in_escrow: TealSourceTemplate,
    pub vote_out_escrow: TealSourceTemplate,
}

#[cfg(test)]
mod tests {
    use crate::{
        dependencies,
        testing::{flow::create_project::create_project_flow, test_data::project_specs},
        testing::{network_test_util::reset_network, test_data::creator},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_create_project_flow() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();

        // UI
        let specs = project_specs();

        let project = create_project_flow(&algod, &creator, &specs).await?;

        // UI
        println!("Submitted create project txs, project: {:?}", project);

        let creator_infos = algod.account_information(&creator.address()).await?;
        let created_assets = creator_infos.created_assets;

        assert_eq!(created_assets.len(), 2);

        println!("created_assets {:?}", created_assets);

        // created asset checks
        assert_eq!(created_assets[0].params.creator, creator.address());
        assert_eq!(created_assets[1].params.creator, creator.address());
        // name matches specs
        assert_eq!(
            created_assets[0].params.name,
            Some(project.specs.shares.token_name.clone())
        );
        assert_eq!(
            created_assets[1].params.name,
            Some(format!("{}v", project.specs.shares.token_name.clone()))
        );
        // unit matches specs
        assert_eq!(
            created_assets[0].params.unit_name,
            Some(project.specs.shares.token_name.clone())
        );
        assert_eq!(
            created_assets[1].params.unit_name,
            Some(format!("{}v", project.specs.shares.token_name.clone()))
        );
        assert_eq!(specs.shares.count, created_assets[0].params.total);
        assert_eq!(specs.shares.count, created_assets[1].params.total);
        let creator_assets = creator_infos.assets;
        // creator sent all the assets to the escrow (during project creation): has 0
        assert_eq!(2, creator_assets.len()); // not opted-out (TODO maybe do this, no reason for creator to be opted in in the investor assets) so still there
        assert_eq!(0, creator_assets[0].amount);
        assert_eq!(0, creator_assets[1].amount);

        // investing escrow funding checks
        let escrow = project.invest_escrow;
        let escrow_infos = algod.account_information(&escrow.address).await?;
        // TODO refactor and check min algos balance
        let escrow_held_assets = escrow_infos.assets;
        assert_eq!(escrow_held_assets.len(), 2);
        assert_eq!(escrow_held_assets[0].asset_id, project.shares_asset_id);
        assert_eq!(escrow_held_assets[0].amount, project.specs.shares.count);
        assert_eq!(escrow_held_assets[1].asset_id, project.votes_asset_id);
        assert_eq!(escrow_held_assets[1].amount, project.specs.shares.count);

        // staking escrow funding checks
        let staking_escrow = project.staking_escrow;
        let staking_escrow_infos = algod.account_information(&staking_escrow.address).await?;
        let staking_escrow_held_assets = staking_escrow_infos.assets;
        // TODO refactor and check min algos balance
        assert_eq!(staking_escrow_held_assets.len(), 2);
        assert_eq!(
            staking_escrow_held_assets[0].asset_id,
            project.shares_asset_id
        );
        assert_eq!(staking_escrow_held_assets[0].amount, 0); // nothing staked yet
        assert_eq!(
            staking_escrow_held_assets[1].asset_id,
            project.votes_asset_id
        );
        assert_eq!(staking_escrow_held_assets[1].amount, 0); // nothing staked yet

        Ok(())
    }
}

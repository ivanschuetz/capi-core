#[cfg(test)]
use crate::flows::create_project::storage::load_project::ProjectId;
#[cfg(test)]
use crate::flows::create_project::storage::{
    creator_investor_setup::{
        creator_investor_setup, submit_creator_investor_setup, CreatorInvestorSetupSigned,
    },
    save_project::{
        save_project_and_optin_to_app, submit_save_project_and_optin_to_app, SaveProjectSigned,
    },
};

#[cfg(test)]
use crate::flows::create_project::{
    create_project::{create_project_txs, submit_create_project},
    create_project_specs::CreateProjectSpecs,
    model::{CreateProjectSigned, Project},
    setup::create_assets::{create_investor_assets_txs, submit_create_assets},
};
#[cfg(test)]
use crate::{
    flows::create_project::create_project::{Escrows, Programs},
    network_util::wait_for_pending_transaction,
    teal::{load_teal, load_teal_template},
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::{anyhow, Result};

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct CreateProjectFlowRes {
    pub project: Project,
    pub project_id: ProjectId,
}

#[cfg(test)]
pub async fn create_project_flow(
    algod: &Algod,
    creator: &Account,
    specs: &CreateProjectSpecs,
    precision: u64,
) -> Result<CreateProjectFlowRes> {
    // Create asset first: id needed in app template
    let create_assets_txs =
        create_investor_assets_txs(&algod, &creator.address(), &specs.shares).await?;

    // UI
    let signed_create_shares_tx = creator.sign_transaction(&create_assets_txs.create_shares_tx)?;

    let create_assets_res = submit_create_assets(algod, &signed_create_shares_tx).await?;

    let programs = programs()?;

    // Rest of create project txs
    let to_sign = create_project_txs(
        &algod,
        specs,
        creator.address(),
        create_assets_res.shares_id,
        programs,
        precision,
    )
    .await?;

    // UI
    let mut signed_funding_txs = vec![];
    for tx in to_sign.escrow_funding_txs {
        signed_funding_txs.push(creator.sign_transaction(&tx)?);
    }
    let signed_create_app_tx = creator.sign_transaction(&to_sign.create_app_tx)?;

    let signed_xfer_shares_to_invest_escrow =
        creator.sign_transaction(&to_sign.xfer_shares_to_invest_escrow)?;

    // Create the asset (submit signed tx) and generate escrow funding tx
    // Note that the escrow is generated after the asset, because it uses the asset id (in teal, inserted with template)

    let create_res = submit_create_project(
        &algod,
        CreateProjectSigned {
            specs: to_sign.specs,
            creator: creator.address(),
            shares_asset_id: create_assets_res.shares_id,
            escrow_funding_txs: signed_funding_txs,
            optin_txs: to_sign.optin_txs,
            create_app_tx: signed_create_app_tx,
            xfer_shares_to_invest_escrow: signed_xfer_shares_to_invest_escrow,
            invest_escrow: to_sign.invest_escrow,
            staking_escrow: to_sign.staking_escrow,
            central_escrow: to_sign.central_escrow,
            customer_escrow: to_sign.customer_escrow,
        },
    )
    .await?;

    log::debug!("Created project: {:?}", create_res.project);

    let save_res =
        save_project_and_optin_to_app(algod, &creator.address(), &create_res.project).await?;
    let signed_app_optin = creator.sign_transaction(&save_res.app_optin_tx)?;
    let signed_save_project = creator.sign_transaction(&save_res.save_project_tx)?;

    let submit_save_project_tx_id = submit_save_project_and_optin_to_app(
        &algod,
        SaveProjectSigned {
            app_optin_tx: signed_app_optin,
            save_project_tx: signed_save_project,
        },
    )
    .await?;

    let project_id = ProjectId(submit_save_project_tx_id.clone());

    let _ = wait_for_pending_transaction(&algod, &submit_save_project_tx_id)
        .await?
        .ok_or(anyhow!("Couldn't get pending tx"))?;

    let creator_investor_setup = creator_investor_setup(
        &algod,
        &creator.address(),
        create_res.project.central_app_id,
        create_res.project.shares_asset_id,
        &project_id,
        &create_res.project,
    )
    .await?;
    let signed_investor_app_setup_tx =
        creator.sign_transaction(&creator_investor_setup.investor_app_setup_tx)?;
    let singed_stake_shares_tx =
        creator.sign_transaction(&creator_investor_setup.stake_shares_tx)?;
    let submit_creator_investor_setup_tx_id = submit_creator_investor_setup(
        &algod,
        CreatorInvestorSetupSigned {
            investor_app_setup_tx: signed_investor_app_setup_tx,
            stake_shares_tx: singed_stake_shares_tx,
        },
    )
    .await?;

    log::debug!(
        "Creator investor setup tx id: {:?}",
        submit_creator_investor_setup_tx_id
    );
    // Waiting for this tx shouldn't be needed for most flows (only if depending on the creator doing investor's actions)
    // Here it's ok as it's only for testing and with dev mode it's not expensive

    let _ = wait_for_pending_transaction(&algod, &submit_creator_investor_setup_tx_id)
        .await?
        .ok_or(anyhow!("Couldn't get pending tx"))?;

    Ok(CreateProjectFlowRes {
        project: create_res.project,
        project_id,
    })
}

#[cfg(test)]
pub fn programs() -> Result<Programs> {
    Ok(Programs {
        central_app_approval: load_teal_template("app_central_approval")?,
        central_app_clear: load_teal("app_central_clear")?,
        escrows: Escrows {
            central_escrow: load_teal_template("central_escrow")?,
            customer_escrow: load_teal_template("customer_escrow")?,
            invest_escrow: load_teal_template("investing_escrow")?,
            staking_escrow: load_teal_template("staking_escrow")?,
        },
    })
}

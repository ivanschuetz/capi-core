#[cfg(test)]
use crate::flows::create_project::{
    logic::{create_project_txs, submit_create_project},
    model::{CreateProjectSigned, CreateProjectSpecs, Project},
    setup::create_assets::{create_investor_assets_txs, submit_create_assets},
};

#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn create_project_flow(
    algod: &Algod,
    creator: &Account,
    specs: &CreateProjectSpecs,
    withdrawal_slots: u64,
) -> Result<Project> {
    use crate::{
        flows::create_project::logic::Programs,
        teal::{load_teal, load_teal_template},
    };

    // Create asset first: id needed in app template
    let create_assets_txs =
        create_investor_assets_txs(&algod, &creator.address(), &specs.shares).await?;

    // UI
    let signed_create_shares_tx = creator.sign_transaction(&create_assets_txs.create_shares_tx)?;
    let signed_create_votes_tx = creator.sign_transaction(&create_assets_txs.create_votes_tx)?;

    let create_assets_res =
        submit_create_assets(algod, &signed_create_shares_tx, &signed_create_votes_tx).await?;

    let programs = Programs {
        central_app_approval: load_teal_template("app_central_approval")?,
        central_app_clear: load_teal("app_central_clear")?,
        central_escrow: load_teal_template("central_escrow")?,
        customer_escrow: load_teal_template("customer_escrow")?,
        invest_escrow: load_teal_template("investing_escrow")?,
        staking_escrow: load_teal_template("staking_escrow")?,
        vote_in_escrow: load_teal_template("voting_in_escrow")?,
        vote_out_escrow: load_teal_template("voting_out_escrow")?,
        withdrawal_slot_approval: load_teal_template("withdrawal_slot_approval")?,
        withdrawal_slot_clear: load_teal("withdrawal_slot_clear")?,
    };

    // Rest of create project txs
    let to_sign = create_project_txs(
        &algod,
        specs,
        creator.address(),
        create_assets_res.shares_id,
        create_assets_res.votes_id,
        programs,
        withdrawal_slots,
    )
    .await?;

    // UI
    let mut signed_funding_txs = vec![];
    for tx in to_sign.escrow_funding_txs {
        signed_funding_txs.push(creator.sign_transaction(&tx)?);
    }
    let signed_create_app_tx = creator.sign_transaction(&to_sign.create_app_tx)?;
    let mut signed_withdrawal_slot_txs = vec![];
    for tx in to_sign.create_withdrawal_slots_txs {
        signed_withdrawal_slot_txs.push(creator.sign_transaction(&tx)?);
    }

    let signed_xfer_shares_to_invest_escrow =
        creator.sign_transaction(&to_sign.xfer_shares_to_invest_escrow)?;
    let signed_xfer_votes_to_invest_escrow =
        creator.sign_transaction(&to_sign.xfer_votes_to_invest_escrow)?;

    // Create the asset (submit signed tx) and generate escrow funding tx
    // Note that the escrow is generated after the asset, because it uses the asset id (in teal, inserted with template)
    let create_res = submit_create_project(
        &algod,
        CreateProjectSigned {
            specs: to_sign.specs,
            creator: creator.address(),
            shares_asset_id: create_assets_res.shares_id,
            votes_asset_id: create_assets_res.votes_id,
            escrow_funding_txs: signed_funding_txs,
            optin_txs: to_sign.optin_txs,
            create_app_tx: signed_create_app_tx,
            create_withdrawal_slots_txs: signed_withdrawal_slot_txs,
            xfer_shares_to_invest_escrow: signed_xfer_shares_to_invest_escrow,
            xfer_votes_to_invest_escrow: signed_xfer_votes_to_invest_escrow,
            invest_escrow: to_sign.invest_escrow,
            staking_escrow: to_sign.staking_escrow,
            central_escrow: to_sign.central_escrow,
            customer_escrow: to_sign.customer_escrow,
            votein_escrow: to_sign.votein_escrow,
            vote_out_escrow: to_sign.vote_out_escrow,
        },
    )
    .await?;

    println!("Created project: {:?}", create_res.project);

    Ok(create_res.project)
}

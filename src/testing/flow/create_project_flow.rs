#[cfg(test)]
use crate::capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps;
#[cfg(test)]
use crate::flows::create_project::storage::load_project::ProjectId;
#[cfg(test)]
use crate::flows::create_project::storage::save_project::{
    save_project, submit_save_project, SaveProjectSigned,
};
#[cfg(test)]
use crate::flows::create_project::{
    create_project::{create_project_txs, submit_create_project, CapiPrograms},
    create_project_specs::CreateProjectSpecs,
    model::{CreateProjectSigned, Project},
    setup::create_shares::{create_assets, submit_create_assets, CrateDaoAssetsSigned},
};
#[cfg(test)]
use crate::funds::FundsAssetId;
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
    funds_asset_id: FundsAssetId,
    precision: u64,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<CreateProjectFlowRes> {
    let programs = programs()?;

    // Create asset first: id needed in app template
    let create_assets_txs = create_assets(
        &algod,
        &creator.address(),
        &specs,
        &programs,
        precision,
        capi_deps,
    )
    .await?;

    // UI
    let signed_create_shares_tx = creator.sign_transaction(&create_assets_txs.create_shares_tx)?;
    let signed_create_app_tx = creator.sign_transaction(&create_assets_txs.create_app_tx)?;

    let create_assets_res = submit_create_assets(
        algod,
        &CrateDaoAssetsSigned {
            create_shares: signed_create_shares_tx,
            create_app: signed_create_app_tx,
        },
    )
    .await?;

    // Rest of create project txs
    let to_sign = create_project_txs(
        &algod,
        specs,
        creator.address(),
        create_assets_res.shares_asset_id,
        funds_asset_id,
        programs,
        precision,
        create_assets_res.app_id,
        capi_deps,
    )
    .await?;

    // UI
    let mut signed_funding_txs = vec![];
    for tx in to_sign.escrow_funding_txs {
        signed_funding_txs.push(creator.sign_transaction(&tx)?);
    }
    let signed_setup_app_tx = creator.sign_transaction(&to_sign.setup_app_tx)?;

    let signed_xfer_shares_to_invest_escrow =
        creator.sign_transaction(&to_sign.xfer_shares_to_invest_escrow)?;

    // Create the asset (submit signed tx) and generate escrow funding tx
    // Note that the escrow is generated after the asset, because it uses the asset id (in teal, inserted with template)

    let create_res = submit_create_project(
        &algod,
        CreateProjectSigned {
            specs: to_sign.specs,
            creator: creator.address(),
            shares_asset_id: create_assets_res.shares_asset_id,
            funds_asset_id: funds_asset_id.clone(),
            escrow_funding_txs: signed_funding_txs,
            optin_txs: to_sign.optin_txs,
            setup_app_tx: signed_setup_app_tx,
            xfer_shares_to_invest_escrow: signed_xfer_shares_to_invest_escrow,
            invest_escrow: to_sign.invest_escrow,
            locking_escrow: to_sign.locking_escrow,
            central_escrow: to_sign.central_escrow,
            customer_escrow: to_sign.customer_escrow,
            central_app_id: create_assets_res.app_id,
        },
    )
    .await?;

    log::debug!("Created project: {:?}", create_res.project);

    let save_res = save_project(algod, &creator.address(), &create_res.project).await?;
    let signed_save_project = creator.sign_transaction(&save_res.tx)?;

    let submit_save_project_tx_id = submit_save_project(
        &algod,
        SaveProjectSigned {
            tx: signed_save_project,
        },
    )
    .await?;

    let project_id = ProjectId(submit_save_project_tx_id.clone());

    let _ = wait_for_pending_transaction(&algod, &submit_save_project_tx_id)
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
            locking_escrow: load_teal_template("locking_escrow")?,
        },
    })
}

#[cfg(test)]
pub fn capi_programs() -> Result<CapiPrograms> {
    Ok(CapiPrograms {
        app_approval: load_teal_template("app_capi_approval")?,
        app_clear: load_teal("app_capi_clear")?,
        escrow: load_teal_template("capi_escrow")?,
    })
}

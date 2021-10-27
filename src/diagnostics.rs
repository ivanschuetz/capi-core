use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
};
use anyhow::Result;

use crate::{
    central_app_state::{
        already_harvested_local_state_or_err, shares_local_state_or_err,
        total_received_amount_global_state_or_err,
    },
    flows::create_project::model::Project,
};

pub async fn harvest_diagnostics(
    algod: &Algod,
    investor: &Address,
    project: &Project,
) -> Result<HarvestDiagnostics> {
    let central_app = algod
        .application_information(project.central_app_id)
        .await?;
    let central_total_received = total_received_amount_global_state_or_err(&central_app)?;

    let investor_infos = algod.account_information(investor).await?;
    let already_harvested = already_harvested_local_state_or_err(
        &investor_infos.apps_local_state,
        project.central_app_id,
    )?;

    let central_balance = algod
        .account_information(&project.central_escrow.address)
        .await?
        .amount;

    let customer_escrow_balance = algod
        .account_information(&project.customer_escrow.address)
        .await?
        .amount;

    let investor_share_count =
        shares_local_state_or_err(&investor_infos.apps_local_state, project.central_app_id)?;

    Ok(HarvestDiagnostics {
        central_total_received,
        already_harvested,
        central_balance,
        customer_escrow_balance,
        investor_balance: investor_infos.amount,
        investor_share_count,
    })
}

pub struct HarvestDiagnostics {
    pub central_total_received: MicroAlgos,
    pub already_harvested: MicroAlgos,
    pub central_balance: MicroAlgos,
    pub customer_escrow_balance: MicroAlgos,
    pub investor_balance: MicroAlgos,
    pub investor_share_count: u64,
}

pub async fn log_harvest_diagnostics(
    algod: &Algod,
    investor: &Address,
    project: &Project,
) -> Result<()> {
    let diag = harvest_diagnostics(algod, investor, project).await?;

    log::info!("//////////////////////////////////////////////////////////");
    log::info!("// harvest diagnostics");
    log::info!("//////////////////////////////////////////////////////////");

    log::info!("central_total_received: {}", diag.central_total_received);
    log::info!("already_harvested: {}", diag.already_harvested);
    log::info!("central_balance: {}", diag.central_balance);
    log::info!("customer_escrow_balance: {}", diag.customer_escrow_balance);
    log::info!("investor_balance: {}", diag.investor_balance);
    log::info!("investor_share_count: {}", diag.investor_share_count);

    log::info!("//////////////////////////////////////////////////////////");
    log::info!("//////////////////////////////////////////////////////////");

    Ok(())
}

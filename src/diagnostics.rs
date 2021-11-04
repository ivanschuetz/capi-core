use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
};
use anyhow::{Error, Result};

use crate::{
    flows::create_project::model::Project,
    state::central_app_state::{central_global_state, central_investor_state},
};

pub async fn harvest_diagnostics(
    algod: &Algod,
    investor: &Address,
    project: &Project,
) -> Result<HarvestDiagnostics> {
    let central_total_received = central_global_state(algod, project.central_app_id)
        .await?
        .received;
    let central_investor_state = central_investor_state(algod, investor, project.central_app_id)
        .await
        .map_err(Error::msg)?;

    let investor_infos = algod.account_information(investor).await?;

    let central_balance = algod
        .account_information(&project.central_escrow.address)
        .await?
        .amount;

    let customer_escrow_balance = algod
        .account_information(&project.customer_escrow.address)
        .await?
        .amount;

    Ok(HarvestDiagnostics {
        central_total_received,
        already_harvested: central_investor_state.harvested,
        central_balance,
        customer_escrow_balance,
        investor_balance: investor_infos.amount,
        investor_share_count: central_investor_state.shares,
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

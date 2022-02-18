use algonaut::{algod::v2::Algod, core::Address};
use anyhow::{Error, Result};

use crate::{
    flows::create_project::{model::Project, share_amount::ShareAmount},
    funds::FundsAmount,
    state::{
        account_state::funds_holdings,
        central_app_state::{central_global_state, central_investor_state},
    },
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

    let central_balance = funds_holdings(
        algod,
        project.central_escrow.address(),
        project.funds_asset_id,
    )
    .await?;

    let customer_escrow_balance = funds_holdings(
        algod,
        project.customer_escrow.address(),
        project.funds_asset_id,
    )
    .await?;

    Ok(HarvestDiagnostics {
        central_total_received,
        already_harvested: central_investor_state.harvested,
        central_balance,
        customer_escrow_balance,
        investor_share_amount: central_investor_state.shares,
    })
}

pub struct HarvestDiagnostics {
    pub central_total_received: FundsAmount,
    pub already_harvested: FundsAmount,
    pub central_balance: FundsAmount,
    pub customer_escrow_balance: FundsAmount,
    // pub investor_balance: Funds,
    pub investor_share_amount: ShareAmount,
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

    log::info!("central_total_received: {:?}", diag.central_total_received);
    log::info!("already_harvested: {:?}", diag.already_harvested);
    log::info!("central_balance: {:?}", diag.central_balance);
    log::info!(
        "customer_escrow_balance: {:?}",
        diag.customer_escrow_balance
    );
    log::info!("investor_share_count: {:?}", diag.investor_share_amount);

    log::info!("//////////////////////////////////////////////////////////");
    log::info!("//////////////////////////////////////////////////////////");

    Ok(())
}

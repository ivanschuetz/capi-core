use algonaut::{algod::v2::Algod, core::Address};
use anyhow::{Error, Result};

use crate::{
    flows::create_dao::{model::Dao, share_amount::ShareAmount},
    funds::FundsAmount,
    state::{
        account_state::funds_holdings,
        central_app_state::{dao_global_state, dao_investor_state},
    },
};

pub async fn claim_diagnostics(
    algod: &Algod,
    investor: &Address,
    dao: &Dao,
) -> Result<ClaimDiagnostics> {
    let central_total_received = dao_global_state(algod, dao.app_id).await?.received;
    let central_investor_state = dao_investor_state(algod, investor, dao.app_id)
        .await
        .map_err(Error::msg)?;

    let central_balance =
        funds_holdings(algod, dao.central_escrow.address(), dao.funds_asset_id).await?;

    let customer_escrow_balance =
        funds_holdings(algod, dao.customer_escrow.address(), dao.funds_asset_id).await?;

    Ok(ClaimDiagnostics {
        central_total_received,
        already_claimed: central_investor_state.claimed,
        central_balance,
        customer_escrow_balance,
        investor_share_amount: central_investor_state.shares,
    })
}

pub struct ClaimDiagnostics {
    pub central_total_received: FundsAmount,
    pub already_claimed: FundsAmount,
    pub central_balance: FundsAmount,
    pub customer_escrow_balance: FundsAmount,
    // pub investor_balance: Funds,
    pub investor_share_amount: ShareAmount,
}

pub async fn log_claim_diagnostics(algod: &Algod, investor: &Address, dao: &Dao) -> Result<()> {
    let diag = claim_diagnostics(algod, investor, dao).await?;

    log::info!("//////////////////////////////////////////////////////////");
    log::info!("// claim diagnostics");
    log::info!("//////////////////////////////////////////////////////////");

    log::info!("central_total_received: {:?}", diag.central_total_received);
    log::info!("already_claimed: {:?}", diag.already_claimed);
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

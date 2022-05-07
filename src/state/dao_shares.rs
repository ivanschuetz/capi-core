use super::{
    account_state::asset_holdings,
    dao_app_state::{dao_global_state, CentralAppGlobalState},
};
use crate::flows::create_dao::{share_amount::ShareAmount, storage::load_dao::DaoAppId};
use algonaut::{algod::v2::Algod, core::to_app_address};
use anyhow::{anyhow, Result};

/// Shares on the DAO's app escrow
pub struct DaoShareHoldings {
    /// Locked: we know this amount via locked global state, which is updated each time shares are locked / unlocked
    /// It's guaranteed to be <= total (there's an error querying this otherwise (using the functions in this file))
    pub locked: ShareAmount,
    /// Not locked: can be purchased by investors
    /// This is the result of subtracting the locked amount from the app's escrow holdings.
    pub available: ShareAmount,
}

impl DaoShareHoldings {
    /// The total amount of shares in the escrow
    pub fn total(&self) -> ShareAmount {
        ShareAmount::new(self.locked.val() + self.available.val())
    }
}

pub async fn dao_shares(
    algod: &Algod,
    app_id: DaoAppId,
    shares_id: u64,
) -> Result<DaoShareHoldings> {
    let holdings = asset_holdings(algod, &to_app_address(app_id.0), shares_id).await?;
    let locked_shares = dao_global_state(algod, app_id).await?.locked_shares;

    if locked_shares.val() > holdings.0 {
        return Err(anyhow!(
            "Critical: invalid state: locked shares: {locked_shares} > holdings: {holdings}",
        ));
    }

    Ok(DaoShareHoldings {
        locked: locked_shares,
        // unchecked substraction: guard to return error if locked > holdings
        available: ShareAmount::new(holdings.0 - locked_shares.val()),
    })
}

pub async fn dao_shares_with_dao_state(
    algod: &Algod,
    app_id: DaoAppId,
    shares_id: u64,
    dao_state: &CentralAppGlobalState,
) -> Result<DaoShareHoldings> {
    let holdings = ShareAmount(asset_holdings(algod, &to_app_address(app_id.0), shares_id).await?);
    dao_shares_all_pars(holdings, dao_state).await
}

pub async fn dao_shares_with_holdings(
    algod: &Algod,
    app_id: DaoAppId,
    dao_holdings: ShareAmount,
) -> Result<DaoShareHoldings> {
    let global_state = dao_global_state(algod, app_id).await?;
    dao_shares_all_pars(dao_holdings, &global_state).await
}

async fn dao_shares_all_pars(
    dao_holdings: ShareAmount,
    dao_state: &CentralAppGlobalState,
) -> Result<DaoShareHoldings> {
    let locked_shares = dao_state.locked_shares;

    if locked_shares.val() > dao_holdings.val() {
        return Err(anyhow!(
            "Critical: invalid state: locked shares: {locked_shares} > holdings: {dao_holdings}",
        ));
    }

    Ok(DaoShareHoldings {
        locked: locked_shares,
        // unchecked substraction: guard to return error if locked > holdings
        available: ShareAmount::new(dao_holdings.val() - locked_shares.val()),
    })
}

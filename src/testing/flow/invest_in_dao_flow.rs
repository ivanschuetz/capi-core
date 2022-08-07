#[cfg(test)]
pub use test::{invests_flow, invests_optins_flow, InvestInDaoTestFlowRes};

#[cfg(test)]
pub mod test {
    use crate::flows::invest::app_optins::{
        invest_or_locking_app_optin_tx, submit_invest_or_locking_app_optin,
    };
    use crate::flows::{
        create_dao::model::Dao,
        invest::model::InvestResult,
        invest::{
            invest::{invest_txs, submit_invest},
            model::InvestSigned,
        },
    };
    use crate::state::account_state::funds_holdings;
    use crate::testing::network_test_util::TestDeps;
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::{anyhow, Result};
    use mbase::models::funds::FundsAmount;
    use mbase::models::share_amount::ShareAmount;
    use mbase::util::network_util::wait_for_pending_transaction;

    /// opts in to the app (requirement for investing)
    pub async fn invests_optins_flow(algod: &Algod, investor: &Account, dao: &Dao) -> Result<()> {
        // app optins (have to happen before invest_txs, which initializes investor's local state)
        let app_optin_tx = invest_or_locking_app_optin_tx(algod, dao, &investor.address()).await?;

        let app_optin_signed_tx = investor.sign_transaction(app_optin_tx)?;

        let app_optin_tx_id =
            submit_invest_or_locking_app_optin(algod, app_optin_signed_tx.clone()).await?;
        wait_for_pending_transaction(&algod, &app_optin_tx_id).await?;

        Ok(())
    }

    // A user buys some shares
    // Resets the network
    pub async fn invests_flow(
        td: &TestDeps,
        investor: &Account,
        buy_share_amount: ShareAmount,
        dao: &Dao,
    ) -> Result<InvestInDaoTestFlowRes> {
        let algod = &td.algod;

        // remember initial investor's funds
        let investor_initial_amount =
            funds_holdings(algod, &investor.address(), td.funds_asset_id).await?;
        // remember initial central escrow's funds
        let app_initial_amount =
            funds_holdings(algod, &dao.app_address(), td.funds_asset_id).await?;

        let to_sign = invest_txs(
            &algod,
            &dao,
            &investor.address(),
            dao.app_id,
            dao.shares_asset_id,
            buy_share_amount,
            td.funds_asset_id,
            dao.share_price,
        )
        .await?;

        let signed_central_app_setup_tx = investor.sign_transaction(to_sign.app_call)?;
        let signed_shares_optin_tx = investor.sign_transaction(to_sign.shares_asset_optin_tx)?;
        let signed_payment_tx = investor.sign_transaction(to_sign.payment_tx)?;

        let invest_res = submit_invest(
            &algod,
            &InvestSigned {
                dao: to_sign.dao,
                central_app_setup_tx: signed_central_app_setup_tx,
                shares_asset_optin_tx: signed_shares_optin_tx,
                payment_tx: signed_payment_tx,
            },
        )
        .await?;

        // wait for tx to go through (so everything is on chain when returning to caller, e.g. to test)
        // TODO (low prio) should be in the tests rather?

        let _ = wait_for_pending_transaction(&algod, &invest_res.tx_id)
            .await?
            .ok_or(anyhow!("Couldn't get pending tx"))?;

        Ok(InvestInDaoTestFlowRes {
            investor_initial_amount,
            central_escrow_initial_amount: app_initial_amount,
            invest_res,
            dao: dao.to_owned(),
            total_paid_price: to_sign.total_price,
        })
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InvestInDaoTestFlowRes {
        pub investor_initial_amount: FundsAmount,
        pub central_escrow_initial_amount: FundsAmount,
        pub invest_res: InvestResult,
        pub dao: Dao,
        // the price paid for the shares bought in the investment
        pub total_paid_price: FundsAmount,
    }
}

#[cfg(test)]
pub mod test {
    use crate::flows::create_dao::model::Dao;
    use crate::flows::reclaim::reclaim::{reclaim, submit_reclaim, ReclaimSigned};
    use crate::testing::network_test_util::TestDeps;
    use algonaut::transaction::account::Account;
    use anyhow::Result;
    use mbase::models::share_amount::ShareAmount;
    use mbase::util::network_util::wait_for_pending_transaction;

    pub async fn reclaim_flow(
        td: &TestDeps,
        dao: &Dao,
        reclaimer: &Account,
        share_amount: ShareAmount,
    ) -> Result<()> {
        let algod = &td.algod;

        // // remember state
        // let reclaimer_balance_before_claiming =
        //     funds_holdings(algod, &reclaimer.address(), td.funds_asset_id).await?;

        let to_sign = reclaim(
            &algod,
            &reclaimer.address(),
            dao.app_id,
            dao.shares_asset_id,
            share_amount,
            td.funds_asset_id,
        )
        .await?;

        let app_call_tx_signed = reclaimer.sign_transaction(to_sign.app_call_tx)?;
        let shares_xfer_tx_signed = reclaimer.sign_transaction(to_sign.shares_xfer_tx)?;

        let claim_tx_id = submit_reclaim(
            &algod,
            &ReclaimSigned {
                app_call_tx_signed,
                shares_xfer_tx_signed,
            },
        )
        .await?;

        wait_for_pending_transaction(&algod, &claim_tx_id).await?;

        Ok(())
    }
}

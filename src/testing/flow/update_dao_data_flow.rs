#[cfg(test)]
pub use test::update_dao_data_flow;
#[cfg(test)]
pub mod test {
    use crate::algo_helpers::wait_for_p_tx_with_id;
    use crate::flows::create_dao::model::Dao;
    use crate::flows::update_data::update_data::{
        submit_update_data, update_data, UpdatableDaoData, UpdateDaoDataSigned,
    };
    use crate::testing::network_test_util::TestDeps;
    use algonaut::transaction::account::Account;
    use anyhow::Result;

    pub async fn update_dao_data_flow(
        td: &TestDeps,
        dao: &Dao,
        owner: &Account,
        data: &UpdatableDaoData,
    ) -> Result<()> {
        let to_sign = update_data(&td.algod, &owner.address(), dao.app_id, data).await?;

        let update_signed = owner.sign_transaction(to_sign.update)?;
        let increase_min_balance_signed = if let Some(tx) = to_sign.increase_min_balance_tx {
            Some(owner.sign_transaction(tx)?)
        } else {
            None
        };

        let tx_id = submit_update_data(
            &td.algod,
            UpdateDaoDataSigned {
                update: update_signed,
                increase_min_balance_tx: increase_min_balance_signed,
            },
        )
        .await?;

        wait_for_p_tx_with_id(&td.algod, &tx_id).await?;

        Ok(())
    }
}

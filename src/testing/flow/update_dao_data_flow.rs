#[cfg(test)]
pub use test::update_dao_data_flow;
#[cfg(test)]
pub mod test {
    use crate::algo_helpers::send_tx_and_wait;
    use crate::flows::create_dao::model::Dao;
    use crate::flows::update_data::update_data::{update_data, UpdatableDaoData};
    use crate::testing::network_test_util::TestDeps;
    use algonaut::transaction::account::Account;
    use anyhow::Result;

    pub async fn update_dao_data_flow(
        td: &TestDeps,
        dao: &Dao,
        owner: &Account,
        data: &UpdatableDaoData,
    ) -> Result<()> {
        let to_sign = update_data(&td.algod, &owner.address(), dao.central_app_id, data).await?;

        let signed = owner.sign_transaction(to_sign.update)?;
        send_tx_and_wait(&td.algod, &signed).await?;

        Ok(())
    }
}

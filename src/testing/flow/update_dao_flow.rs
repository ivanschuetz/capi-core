#[cfg(test)]
pub use test::update_dao_flow;
#[cfg(test)]
pub mod test {
    use crate::algo_helpers::send_tx_and_wait;
    use crate::flows::create_dao::model::Dao;
    use crate::flows::update_app::update::update;
    use crate::testing::network_test_util::TestDeps;
    use algonaut::transaction::account::Account;
    use anyhow::Result;
    use mbase::teal::TealSource;

    pub async fn update_dao_flow(
        td: &TestDeps,
        dao: &Dao,
        owner: &Account,
        approval: TealSource,
        clear: TealSource,
    ) -> Result<()> {
        let compiled_approval = td.algod.compile_teal(&approval.0).await?;
        let compiled_clear = td.algod.compile_teal(&clear.0).await?;

        let to_sign = update(
            &td.algod,
            &owner.address(),
            dao.app_id,
            compiled_approval,
            compiled_clear,
        )
        .await?;

        let signed = owner.sign_transaction(to_sign.update)?;
        send_tx_and_wait(&td.algod, &signed).await?;

        Ok(())
    }
}

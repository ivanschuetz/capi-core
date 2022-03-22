#[cfg(test)]
pub use test::update_capi_flow;

#[cfg(test)]
pub mod test {
    use crate::algo_helpers::send_tx_and_wait;
    use crate::capi_asset::capi_app_id::CapiAppId;
    use crate::capi_asset::update_app::update::update;
    use crate::teal::TealSource;
    use crate::testing::network_test_util::TestDeps;
    use algonaut::transaction::account::Account;
    use anyhow::Result;

    pub async fn update_capi_flow(
        td: &TestDeps,
        app_id: CapiAppId,
        owner: &Account,
        approval: TealSource,
        clear: TealSource,
    ) -> Result<()> {
        let to_sign = update(&td.algod, &owner.address(), app_id, approval, clear).await?;

        let signed = owner.sign_transaction(to_sign.update)?;
        send_tx_and_wait(&td.algod, &signed).await?;

        Ok(())
    }
}

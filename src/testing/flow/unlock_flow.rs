#[cfg(test)]
pub use test::unlock_flow;

#[cfg(test)]
pub mod test {
    use crate::flows::create_dao::model::Dao;
    use crate::flows::create_dao::storage::load_dao::TxId;
    use crate::flows::unlock::unlock::unlock;
    use crate::flows::unlock::unlock::{submit_unlock, UnlockSigned};
    use algonaut::{algod::v2::Algod, transaction::account::Account};
    use anyhow::Result;

    pub async fn unlock_flow(algod: &Algod, dao: &Dao, investor: &Account) -> Result<TxId> {
        let to_sign = unlock(&algod, investor.address(), dao.app_id, dao.shares_asset_id).await?;

        let signed_central_app_optout = investor.sign_transaction(to_sign.central_app_optout_tx)?;

        let tx_id = submit_unlock(
            algod,
            UnlockSigned {
                central_app_optout_tx: signed_central_app_optout,
            },
        )
        .await?;

        Ok(tx_id)
    }
}

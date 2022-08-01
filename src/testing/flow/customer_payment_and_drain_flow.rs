#[cfg(test)]
pub use test::{customer_payment_and_drain_flow, drain_flow, CustomerPaymentAndDrainFlowRes};

#[cfg(test)]
pub mod test {
    use crate::{
        flows::create_dao::model::Dao,
        flows::drain::drain::{
            drain, submit_drain, to_drain_amounts, DaoAndCapiDrainAmounts, DrainSigned,
        },
        flows::pay_dao::pay_dao::{pay_dao_app, submit_pay_dao, PayDaoSigned},
        testing::network_test_util::TestDeps,
    };
    use algonaut::{
        algod::v2::Algod,
        core::MicroAlgos,
        transaction::{account::Account, Transaction},
    };
    use anyhow::Result;
    use mbase::{
        models::{
            dao_app_id::DaoAppId,
            funds::{FundsAmount, FundsAssetId},
            tx_id::TxId,
        },
        util::network_util::wait_for_pending_transaction,
    };

    pub async fn customer_payment_and_drain_flow(
        td: &TestDeps,
        dao: &Dao,
        customer_payment_amount: FundsAmount,
        drainer: &Account,
    ) -> Result<CustomerPaymentAndDrainFlowRes> {
        let algod = &td.algod;

        // Customer sends a payment
        let customer_payment_tx_id = send_payment_to_app(
            algod,
            &td.customer,
            dao.app_id,
            td.funds_asset_id,
            customer_payment_amount,
        )
        .await?;
        wait_for_pending_transaction(&algod, &customer_payment_tx_id).await?;

        drain_flow(td, &drainer, dao).await
    }

    pub async fn drain_flow(
        td: &TestDeps,
        drainer: &Account,
        dao: &Dao,
    ) -> Result<CustomerPaymentAndDrainFlowRes> {
        let algod = &td.algod;

        let initial_drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        let drain_amounts = to_drain_amounts(
            algod,
            td.dao_deps().escrow_percentage,
            dao.funds_asset_id,
            dao.app_id,
        )
        .await?;

        let drain_to_sign = drain(
            &algod,
            &drainer.address(),
            dao.app_id,
            dao.funds_asset_id,
            &td.dao_deps(),
            &drain_amounts,
        )
        .await?;

        let app_call_tx_signed = drainer.sign_transaction(drain_to_sign.app_call_tx)?;

        let drain_tx_id = submit_drain(
            &algod,
            &DrainSigned {
                app_call_tx_signed: app_call_tx_signed.clone(),
            },
        )
        .await?;

        wait_for_pending_transaction(&algod, &drain_tx_id).await?;

        Ok(CustomerPaymentAndDrainFlowRes {
            dao: dao.to_owned(),
            initial_drainer_balance,
            app_call_tx: app_call_tx_signed.transaction,
            drained_amounts: drain_amounts,
        })
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CustomerPaymentAndDrainFlowRes {
        pub dao: Dao,
        pub initial_drainer_balance: MicroAlgos,
        pub app_call_tx: Transaction,
        pub drained_amounts: DaoAndCapiDrainAmounts,
    }

    // Simulate a payment to the dao address
    async fn send_payment_to_app(
        algod: &Algod,
        customer: &Account,
        app_id: DaoAppId,
        funds_asset_id: FundsAssetId,
        amount: FundsAmount,
    ) -> Result<TxId> {
        let tx = pay_dao_app(algod, &customer.address(), app_id, funds_asset_id, amount)
            .await?
            .tx;
        let signed_tx = customer.sign_transaction(tx)?;
        let tx_id = submit_pay_dao(algod, PayDaoSigned { tx: signed_tx }).await?;
        log::debug!("Customer payment tx id: {:?}", tx_id);
        Ok(tx_id)
    }
}

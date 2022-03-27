#[cfg(test)]
pub use test::{customer_payment_and_drain_flow, drain_flow, CustomerPaymentAndDrainFlowRes};

#[cfg(test)]
pub mod test {
    use crate::funds::FundsAmount;
    use crate::funds::FundsAssetId;
    use crate::{
        flows::create_dao::model::Dao,
        flows::create_dao::storage::load_dao::TxId,
        flows::drain::drain::{
            drain_amounts, drain_customer_escrow, submit_drain_customer_escrow,
            DaoAndCapiDrainAmounts, DrainCustomerEscrowSigned,
        },
        flows::pay_dao::pay_dao::{pay_dao, submit_pay_dao, PayDaoSigned},
        network_util::wait_for_pending_transaction,
        state::account_state::funds_holdings,
        testing::network_test_util::TestDeps,
    };
    use algonaut::{
        algod::v2::Algod,
        core::{Address, MicroAlgos},
        transaction::{account::Account, Transaction},
    };
    use anyhow::Result;

    pub async fn customer_payment_and_drain_flow(
        td: &TestDeps,
        dao: &Dao,
        customer_payment_amount: FundsAmount,
        drainer: &Account,
    ) -> Result<CustomerPaymentAndDrainFlowRes> {
        let algod = &td.algod;

        // double check precondition: customer escrow has no funds
        let customer_escrow_holdings =
            funds_holdings(algod, dao.customer_escrow.address(), td.funds_asset_id).await?;
        assert_eq!(FundsAmount::new(0), customer_escrow_holdings);

        // Customer sends a payment
        let customer_payment_tx_id = send_payment_to_customer_escrow(
            algod,
            &td.customer,
            dao.customer_escrow.address(),
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

        let drain_amounts = drain_amounts(
            algod,
            td.dao_deps().escrow_percentage,
            dao.funds_asset_id,
            &dao.customer_escrow.address(),
        )
        .await?;

        let drain_to_sign = drain_customer_escrow(
            &algod,
            &drainer.address(),
            dao.app_id,
            dao.funds_asset_id,
            &td.dao_deps(),
            &dao.customer_escrow,
            &dao.central_escrow.address(),
            &drain_amounts,
        )
        .await?;

        let app_call_tx_signed = drainer.sign_transaction(drain_to_sign.app_call_tx)?;
        let capi_app_call_tx_signed = drainer.sign_transaction(drain_to_sign.capi_app_call_tx)?;

        let drain_tx_id = submit_drain_customer_escrow(
            &algod,
            &DrainCustomerEscrowSigned {
                drain_tx: drain_to_sign.drain_tx,
                capi_share_tx: drain_to_sign.capi_share_tx,
                app_call_tx_signed: app_call_tx_signed.clone(),
                capi_app_call_tx_signed: capi_app_call_tx_signed.clone(),
            },
        )
        .await?;

        wait_for_pending_transaction(&algod, &drain_tx_id).await?;

        Ok(CustomerPaymentAndDrainFlowRes {
            dao: dao.to_owned(),
            initial_drainer_balance,
            app_call_tx: app_call_tx_signed.transaction,
            capi_app_call_tx: capi_app_call_tx_signed.transaction,
            drained_amounts: drain_amounts,
        })
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CustomerPaymentAndDrainFlowRes {
        pub dao: Dao,
        pub initial_drainer_balance: MicroAlgos,
        pub app_call_tx: Transaction,
        pub capi_app_call_tx: Transaction,
        pub drained_amounts: DaoAndCapiDrainAmounts,
    }

    // Simulate a payment to the "external" dao address
    async fn send_payment_to_customer_escrow(
        algod: &Algod,
        customer: &Account,
        customer_escrow: &Address,
        funds_asset_id: FundsAssetId,
        amount: FundsAmount,
    ) -> Result<TxId> {
        let tx = pay_dao(
            algod,
            &customer.address(),
            customer_escrow,
            funds_asset_id,
            amount,
        )
        .await?
        .tx;
        let signed_tx = customer.sign_transaction(tx)?;
        let tx_id = submit_pay_dao(algod, PayDaoSigned { tx: signed_tx }).await?;
        log::debug!("Customer payment tx id: {:?}", tx_id);
        Ok(tx_id)
    }
}

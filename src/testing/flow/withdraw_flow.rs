#[cfg(test)]
pub use test::withdraw_precs;

#[cfg(test)]
pub mod test {
    use crate::network_util::wait_for_pending_transaction;
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::network_test_util::TestDeps;
    use crate::testing::tests_msig::TestsMsig;
    use crate::{
        flows::{
            create_dao::model::Dao,
            withdraw::withdraw::{submit_withdraw, withdraw, WithdrawSigned, WithdrawalInputs},
        },
        testing::flow::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes,
    };
    use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
    use anyhow::Result;
    use mbase::models::dao_app_id::DaoAppId;
    use mbase::models::funds::{FundsAmount, FundsAssetId};

    /// dao creation,
    /// customer payment + draining to central, to have something to withdraw.
    pub async fn withdraw_precs(
        td: &TestDeps,
        drainer: &Account,
        dao: &Dao,
        pay_and_drain_amount: FundsAmount,
    ) -> Result<WithdrawTestPrecsRes> {
        let algod = &td.algod;

        // customer payment and draining, to have some funds to withdraw

        let drain_res =
            customer_payment_and_drain_flow(&td, &dao, pay_and_drain_amount, &drainer).await?;
        let app_balance_after_drain = algod
            .account_information(&drain_res.dao.app_address())
            .await?
            .amount;

        Ok(WithdrawTestPrecsRes {
            central_escrow_balance_after_drain: app_balance_after_drain,
            drain_res,
        })
    }

    pub async fn withdraw_flow(
        algod: &Algod,
        dao: &Dao,
        withdrawer: &Account,
        amount: FundsAmount,
        app_id: DaoAppId,
    ) -> Result<WithdrawTestFlowRes> {
        // remember state
        let withdrawer_balance_before_withdrawing = algod
            .account_information(&withdrawer.address())
            .await?
            .amount;

        let to_sign = withdraw(
            &algod,
            withdrawer.address(),
            &WithdrawalInputs {
                amount: amount.to_owned(),
                description: "Withdrawing from tests".to_owned(),
            },
            app_id,
            dao.funds_asset_id,
        )
        .await?;

        let withdraw_signed = withdrawer.sign_transaction(to_sign.withdraw_tx)?;

        let withdraw_tx_id = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: withdraw_signed,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &withdraw_tx_id).await?;

        Ok(WithdrawTestFlowRes {
            dao: dao.clone(),
            withdrawer_balance_before_withdrawing,
            withdrawal: amount.to_owned(),
        })
    }

    #[allow(dead_code)] // we might test msig once rekey functionality complete
    pub async fn withdraw_msig_flow(
        algod: &Algod,
        withdrawer: &TestsMsig,
        amount: FundsAmount,
        app_id: DaoAppId,
        funds_asset: FundsAssetId,
    ) -> Result<()> {
        let to_sign = withdraw(
            &algod,
            withdrawer.address().address(),
            &WithdrawalInputs {
                amount: amount.to_owned(),
                description: "Withdrawing from tests".to_owned(),
            },
            app_id,
            funds_asset,
        )
        .await?;

        let withdraw_signed = withdrawer.sign(to_sign.withdraw_tx)?;

        let withdraw_tx_id = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: withdraw_signed,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &withdraw_tx_id).await?;

        Ok(())
    }

    #[allow(dead_code)] // we might test msig once rekey functionality complete
    pub async fn withdraw_incomplete_msig_flow(
        algod: &Algod,
        withdrawer: &TestsMsig,
        amount: FundsAmount,
        app_id: DaoAppId,
        funds_asset: FundsAssetId,
    ) -> Result<()> {
        let to_sign = withdraw(
            &algod,
            withdrawer.address().address(),
            &WithdrawalInputs {
                amount: amount.to_owned(),
                description: "Withdrawing from tests".to_owned(),
            },
            app_id,
            funds_asset,
        )
        .await?;

        let withdraw_signed = withdrawer.sign_incomplete(to_sign.withdraw_tx)?;

        let withdraw_tx_id = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: withdraw_signed,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &withdraw_tx_id).await?;

        Ok(())
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WithdrawTestFlowRes {
        pub dao: Dao,
        pub withdrawer_balance_before_withdrawing: MicroAlgos,
        pub withdrawal: FundsAmount,
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WithdrawTestPrecsRes {
        pub central_escrow_balance_after_drain: MicroAlgos,
        pub drain_res: CustomerPaymentAndDrainFlowRes,
    }
}

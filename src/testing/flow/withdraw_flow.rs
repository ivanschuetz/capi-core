#[cfg(test)]
pub use test::withdraw_precs;

#[cfg(test)]
pub mod test {
    use crate::funds::{FundsAmount, FundsAssetId};
    use crate::network_util::wait_for_pending_transaction;
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::network_test_util::TestDeps;
    use crate::{
        flows::{
            create_project::model::Project,
            withdraw::withdraw::{submit_withdraw, withdraw, WithdrawSigned, WithdrawalInputs},
        },
        testing::flow::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes,
    };
    use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
    use anyhow::Result;

    /// project creation,
    /// customer payment + draining to central, to have something to withdraw.
    pub async fn withdraw_precs(
        td: &TestDeps,
        drainer: &Account,
        project: &Project,
        pay_and_drain_amount: FundsAmount,
    ) -> Result<WithdrawTestPrecsRes> {
        let algod = &td.algod;

        // customer payment and draining, to have some funds to withdraw

        let drain_res =
            customer_payment_and_drain_flow(&td, &project, pay_and_drain_amount, &drainer).await?;
        let central_escrow_balance_after_drain = algod
            .account_information(drain_res.project.central_escrow.address())
            .await?
            .amount;

        Ok(WithdrawTestPrecsRes {
            central_escrow_balance_after_drain,
            drain_res,
        })
    }

    pub async fn withdraw_flow(
        algod: &Algod,
        project: &Project,
        creator: &Account,
        amount: FundsAmount,
        funds_asset_id: FundsAssetId,
    ) -> Result<WithdrawTestFlowRes> {
        // remember state
        let withdrawer_balance_before_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        let to_sign = withdraw(
            &algod,
            creator.address(),
            funds_asset_id,
            &WithdrawalInputs {
                amount: amount.to_owned(),
                description: "Withdrawing from tests".to_owned(),
            },
            &project.central_escrow,
        )
        .await?;

        let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;

        let withdraw_tx_id = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &withdraw_tx_id).await?;

        Ok(WithdrawTestFlowRes {
            project: project.clone(),
            withdrawer_balance_before_withdrawing,
            withdrawal: amount.to_owned(),
        })
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WithdrawTestFlowRes {
        pub project: Project,
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

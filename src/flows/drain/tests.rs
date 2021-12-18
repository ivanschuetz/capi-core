#[cfg(test)]
mod tests {
    use algonaut::{
        core::MicroAlgos,
        transaction::{Transaction, TransactionType},
    };
    use anyhow::{anyhow, Result};
    use data_encoding::BASE64;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::drain::drain::{FIXED_FEE, MIN_BALANCE},
        testing::{
            flow::{
                create_project_flow::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
            },
            network_test_util::reset_network,
            project_general::check_schema,
            test_data::{creator, customer, investor1, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };

    #[test]
    #[serial]
    async fn test_drain() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let customer = customer();

        // UI
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        let customer_payment_amount = MicroAlgos(10 * 1_000_000);

        // flow

        let drain_res = customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            customer_payment_amount,
            &project,
        )
        .await?;

        let customer_escrow_balance = algod
            .account_information(&drain_res.project.customer_escrow.address)
            .await?
            .amount;
        let central_escrow_balance = algod
            .account_information(&drain_res.project.central_escrow.address)
            .await?
            .amount;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        println!(
            "customer_escrow_balance last: {:?}",
            customer_escrow_balance
        );
        println!("central_escrow_balance last: {:?}", central_escrow_balance);
        println!("drainer_balance last: {:?}", drainer_balance);
        // check that customer escrow was drained. Account keeps min balance, and fee (to be able to pay for the harvest tx (before it's funded in the same group))
        assert_eq!(MIN_BALANCE + FIXED_FEE, customer_escrow_balance);
        // check that central escrow has now the funds from customer escrow (funds at creation: MIN_BALANCE + FIXED_FEE)
        assert_eq!(
            MIN_BALANCE + FIXED_FEE + customer_payment_amount,
            central_escrow_balance
        );
        // check that the drainer lost the payment for the draining tx fee, the fee for this payment tx and the app call fee
        assert_eq!(
            drain_res.initial_drainer_balance
                - retrieve_payment_amount_from_tx(&drain_res.pay_fee_tx)?
                - drain_res.pay_fee_tx.fee
                - drain_res.app_call_tx.fee,
            drainer_balance
        );

        // test the global state after drain
        let app = algod
            .application_information(project.central_app_id)
            .await?;
        assert_eq!(1, app.params.global_state.len());
        let key_value = &app.params.global_state[0];
        assert_eq!(BASE64.encode(b"CentralReceivedTotal"), key_value.key);
        assert_eq!(Vec::<u8>::new(), key_value.value.bytes);
        // after drain, the central received total gs is the amount that was drained
        assert_eq!(drain_res.drained_amount.0, key_value.value.uint);
        // values not documented: 1 is byte slice and 2 uint
        // https://forum.algorand.org/t/interpreting-goal-app-read-response/2711
        assert_eq!(2, key_value.value.value_type);
        // double check (_very_ unlikely to be needed)
        check_schema(&app);

        Ok(())
    }

    // TODO (low prio) is there a way to model this in Algonaut so we know what tx type we're dealing with at compile time
    // generics: Transaction<T: TransactionType>? something else?
    // TODO refactor with identical fn in invest test
    fn retrieve_payment_amount_from_tx(tx: &Transaction) -> Result<MicroAlgos> {
        match &tx.txn_type {
            TransactionType::Payment(p) => Ok(p.amount),
            _ => Err(anyhow!(
                "Invalid state: tx is expected to be a payment tx: {:?}",
                tx
            )),
        }
    }
}

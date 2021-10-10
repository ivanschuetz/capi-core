use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CallApplication, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn drain_customer_escrow(
    algod: &Algod,
    drainer: &Address,
    central_app_id: u64,
    customer_escrow: &ContractAccount,
    central_escrow: &ContractAccount,
) -> Result<DrainCustomerEscrowToSign> {
    let params = algod.suggested_transaction_params().await?;
    let customer_escrow_balance = algod
        .account_information(&customer_escrow.address)
        .await?
        .amount;

    let balance_to_drain = customer_escrow_balance - MIN_BALANCE - FIXED_FEE; // leave min balance and "tmp fee amount"

    let drain_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(
            customer_escrow.address,
            central_escrow.address,
            balance_to_drain,
        )
        .build(),
    )
    .build();

    let pay_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(*drainer, customer_escrow.address, FIXED_FEE).build(),
    )
    .build();

    let app_call_tx = &mut drain_app_call_tx(central_app_id, &params, drainer)?;

    TxGroup::assign_group_id(vec![app_call_tx, drain_tx, pay_fee_tx])?;

    let signed_drain_tx = customer_escrow.sign(drain_tx, vec![])?;

    Ok(DrainCustomerEscrowToSign {
        drain_tx: signed_drain_tx,
        pay_fee_tx: pay_fee_tx.clone(),
        app_call_tx: app_call_tx.clone(),
        amount_to_drain: balance_to_drain,
    })
}

pub fn drain_app_call_tx(
    app_id: u64,
    params: &SuggestedTransactionParams,
    sender: &Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(*sender, app_id).build(),
    )
    .build();
    Ok(tx)
}

pub async fn submit_drain_customer_escrow(
    algod: &Algod,
    signed: &DrainCustomerEscrowSigned,
) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.pay_fee_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx_signed.clone(),
    //         signed.drain_tx.clone(),
    //         signed.pay_fee_tx.clone(),
    //     ],
    //     "customer_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.app_call_tx_signed.clone(),
            signed.drain_tx.clone(),
            signed.pay_fee_tx.clone(),
        ])
        .await?;
    println!("Drain customer escrow tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainCustomerEscrowToSign {
    pub drain_tx: SignedTransaction,
    pub pay_fee_tx: Transaction,
    pub app_call_tx: Transaction,
    pub amount_to_drain: MicroAlgos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainCustomerEscrowSigned {
    pub drain_tx: SignedTransaction,
    pub pay_fee_tx: SignedTransaction,
    pub app_call_tx_signed: SignedTransaction,
}

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
        flows::drain::logic::{FIXED_FEE, MIN_BALANCE},
        testing::{
            flow::{
                create_project::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
            },
            network_test_util::reset_network,
            project_general::check_schema,
            test_data::{creator, customer, investor1, project_specs},
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

        let project = create_project_flow(&algod, &creator, &specs).await?;

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

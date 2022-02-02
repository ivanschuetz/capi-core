use crate::flows::create_project::storage::load_project::TxId;
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, contract_account::ContractAccount, tx_group::TxGroup,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

/// Note that this is only for shares that have been bought in the market
/// The investing flow doesn't use this: there's an xfer from the investing account to the staking escrow in the investing tx group
pub async fn stake(
    algod: &Algod,
    investor: Address,
    share_count: u64,
    shares_asset_id: u64,
    central_app_id: u64,
    staking_escrow: &ContractAccount,
) -> Result<StakeToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Central app setup app call (init investor's local state)
    let mut app_call_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(investor, central_app_id).build(),
    )
    .build();

    // Send investor's assets to staking escrow
    let mut shares_xfer_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        TransferAsset::new(
            investor,
            shares_asset_id,
            share_count,
            *staking_escrow.address(),
        )
        .build(),
    )
    .build();

    let txs_for_group = vec![&mut app_call_tx, &mut shares_xfer_tx];
    TxGroup::assign_group_id(txs_for_group)?;

    Ok(StakeToSign {
        central_app_call_setup_tx: app_call_tx.clone(),
        shares_xfer_tx: shares_xfer_tx.clone(),
    })
}

pub async fn submit_stake(algod: &Algod, signed: StakeSigned) -> Result<TxId> {
    let txs = vec![
        signed.central_app_call_setup_tx.clone(),
        signed.shares_xfer_tx_signed.clone(),
    ];
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Stake tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakeToSign {
    pub central_app_call_setup_tx: Transaction,
    pub shares_xfer_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakeSigned {
    pub central_app_call_setup_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
}

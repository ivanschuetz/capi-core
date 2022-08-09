use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, SignedTransaction, Transaction, TransferAsset,
        TxnBuilder,
    },
};
use anyhow::Result;
use mbase::{
    models::{dao_app_id::DaoAppId, share_amount::ShareAmount, tx_id::TxId},
    state::dao_app_state::SignedProspectus,
};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);

/// Note that this is only for shares that have been bought in the market
/// The investing flow doesn't use this: there's an xfer from the investing account to the app escrow in the investing tx group
pub async fn lock(
    algod: &Algod,
    investor: Address,
    share_amount: ShareAmount,
    shares_asset_id: u64,
    app_id: DaoAppId,
) -> Result<LockToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Initialize the corresponding local state in teal
    // it's None since acking prospectus seems to make sense only when buying the shares,
    // which in this case would be either in a third party exchange or when investing here,
    // we've to set it either way, since we expect state to always be completely initialized / have a fixed size
    // note that in the future we might (?) want or need to ack or somehow receive a forwarded prospectus when locking
    let signed_prospectus: Option<SignedProspectus> = None;

    // Central app setup app call (init investor's local state)
    let mut app_call_tx = TxnBuilder::with(
        &params,
        CallApplication::new(investor, app_id.0)
            .app_arguments(vec![
                "lock".as_bytes().to_vec(),
                signed_prospectus
                    .clone()
                    .map(|p| p.url.as_bytes().to_vec())
                    .unwrap_or_default(),
                signed_prospectus
                    .clone()
                    .map(|p| p.hash.as_bytes().to_vec())
                    .unwrap_or_default(),
                signed_prospectus
                    .map(|p| p.timestamp.0.to_be_bytes().to_vec())
                    .unwrap_or_default(),
            ])
            .build(),
    )
    .build()?;

    // Send investor's assets to app escrow
    let mut shares_xfer_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(
            investor,
            shares_asset_id,
            share_amount.val(),
            app_id.address(),
        )
        .build(),
    )
    .build()?;

    let txs_for_group = &mut [&mut app_call_tx, &mut shares_xfer_tx];
    TxGroup::assign_group_id(txs_for_group)?;

    Ok(LockToSign {
        central_app_call_setup_tx: app_call_tx.clone(),
        shares_xfer_tx: shares_xfer_tx.clone(),
    })
}

pub async fn submit_lock(algod: &Algod, signed: LockSigned) -> Result<TxId> {
    log::debug!("calling submit lock..");

    let txs = vec![
        signed.central_app_call_setup_tx.clone(),
        signed.shares_xfer_tx_signed.clone(),
    ];
    // mbase::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();
    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Lock tx id: {:?}", res.tx_id);
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockToSign {
    pub central_app_call_setup_tx: Transaction,
    pub shares_xfer_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockSigned {
    pub central_app_call_setup_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
}

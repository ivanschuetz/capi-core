use crate::teal::{render_template, save_rendered_teal, TealSource, TealSourceTemplate};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        account::ContractAccount, AcceptAsset, Pay, SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;
use serde::Serialize;

/// The votein escrow holds votes for a withdrawal (currently we only support one withdrawal at a time)
/// The creator sends a grouped tx to transfer the votes from votesin to votesout (to be implemented) with the withdrawal tx
/// If there are enough votes in votesin, the group is allowed

async fn create_votein_escrow(
    algod: &Algod,
    source: TealSourceTemplate,
    votes_asset_id: u64,
    votes_threshold_units: u64,
    votes_out_address: Address,
) -> Result<ContractAccount> {
    let escrow = load_votein_escrow(
        algod,
        source,
        votes_asset_id,
        votes_threshold_units,
        votes_out_address,
    )
    .await?;
    Ok(escrow)
}

async fn load_votein_escrow(
    algod: &Algod,
    source: TealSourceTemplate,
    votes_asset_id: u64,
    votes_threshold_units: u64,
    votes_out_address: Address,
) -> Result<ContractAccount> {
    let source = render_votes_in_escrow(
        source,
        votes_asset_id,
        votes_threshold_units,
        votes_out_address,
    )?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

fn render_votes_in_escrow(
    source: TealSourceTemplate,
    votes_asset_id: u64,
    votes_threshold_units: u64,
    votes_out_address: Address,
) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        VotesInEscrowTemplateContext {
            votes_asset_id: votes_asset_id.to_string(),
            votes_threshold_units: votes_threshold_units.to_string(),
            votes_out_address: votes_out_address.to_string(),
        },
    )?;
    save_rendered_teal("voting_in_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

#[derive(Serialize)]
struct VotesInEscrowTemplateContext {
    votes_asset_id: String,
    votes_threshold_units: String,
    votes_out_address: String,
}

pub async fn setup_votein_escrow_txs(
    algod: &Algod,
    source: TealSourceTemplate,
    creator: Address,
    votes_asset_id: u64,
    votes_threshold_units: u64,
    votes_out_address: Address,
) -> Result<SetupVoteInEscrowToSign> {
    let escrow = create_votein_escrow(
        algod,
        source,
        votes_asset_id,
        votes_threshold_units,
        votes_out_address,
    )
    .await?;
    println!("Generated votein escrow address: {:?}", escrow.address);

    let params = algod.suggested_transaction_params().await?;

    // Send some funds to the escrow (min amount to hold asset, pay for opt in tx fee)
    let fund_algos_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(creator, escrow.address, MicroAlgos(1_000_000)).build(),
    )
    .build();

    // Escrow opts in to the vote asset
    let votes_optin_tx = TxnBuilder::with(
        params,
        AcceptAsset::new(escrow.address, votes_asset_id).build(),
    )
    .build();
    // let votes_optin_signed_tx = escrow.sign(votes_optin_tx, vec![])?;

    // TODO is it possible and does it make sense to execute these atomically?,
    // "sc opts in to asset and I send funds to sc"
    // TxGroup::assign_group_id(vec![optin_tx, fund_tx])?;

    Ok(SetupVoteInEscrowToSign {
        escrow,
        escrow_votes_optin_tx: votes_optin_tx,
        escrow_funding_algos_tx: fund_algos_tx,
    })
}

pub async fn submit_votein_setup_escrow(
    algod: &Algod,
    signed: SetupVoteInEscrowSigned,
) -> Result<SubmitVoteInSetupEscrowRes> {
    let fund_escrow_algos_res = algod
        .broadcast_signed_transaction(&signed.funding_algos_tx)
        .await?;
    println!("fund_escrow_algos_res: {:?}", fund_escrow_algos_res);

    let votes_optin_escrow_res = algod
        .broadcast_signed_transaction(&signed.votes_optin_tx)
        .await?;
    println!("votes_optin_escrow_res: {:?}", votes_optin_escrow_res);

    Ok(SubmitVoteInSetupEscrowRes {
        fund_escrow_algos_tx_id: fund_escrow_algos_res.tx_id,
        votes_optin_escrow_algos_tx_id: votes_optin_escrow_res.tx_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupVoteInEscrowToSign {
    pub escrow: ContractAccount,
    // pub escrow_votes_optin_tx: SignedTransaction,
    pub escrow_votes_optin_tx: Transaction,
    pub escrow_funding_algos_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupVoteInEscrowSigned {
    pub escrow: ContractAccount,
    pub votes_optin_tx: SignedTransaction,
    pub funding_algos_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitVoteInSetupEscrowRes {
    pub fund_escrow_algos_tx_id: String,
    pub votes_optin_escrow_algos_tx_id: String,
}

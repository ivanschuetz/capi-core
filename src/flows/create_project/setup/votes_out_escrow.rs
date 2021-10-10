use crate::teal::{render_template, TealSource, TealSourceTemplate};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    model::algod::v2::CompiledTeal,
    transaction::{
        account::ContractAccount, AcceptAsset, Pay, SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;
use serde::Serialize;

pub async fn create_votes_out_escrow_tx(
    algod: &Algod,
    source: TealSourceTemplate,
    creator: Address,
    shares_asset_id: u64,
    votes_asset_id: u64,
    staking_escrow_address: Address,
) -> Result<SetupVoteOutEscrowToSign> {
    println!("Creating app tx..");

    let params = algod.suggested_transaction_params().await?;

    let escrow = create_vote_out_escrow(
        algod,
        source,
        shares_asset_id,
        votes_asset_id,
        staking_escrow_address,
    )
    .await?;
    println!("vote out escrow: {:?}", escrow.address);

    // Send some funds to the escrow (min amount to hold asset, pay for opt in tx fee)
    let fund_algos_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(creator, escrow.address, MicroAlgos(1_000_000)).build(),
    )
    .build();

    // Escrow opts in to the vote asset
    let votes_optin_tx = TxnBuilder::with(
        params.clone(),
        AcceptAsset::new(escrow.address, votes_asset_id).build(),
    )
    .build();
    // let votes_optin_signed_tx = escrow.sign(votes_optin_tx, vec![])?;

    // TODO is it possible and does it make sense to execute these atomically?,
    // "sc opts in to asset and I send funds to sc"
    // TxGroup::assign_group_id(vec![optin_tx, fund_tx])?;

    Ok(SetupVoteOutEscrowToSign {
        escrow,
        escrow_votes_optin_tx: votes_optin_tx,
        escrow_funding_algos_tx: fund_algos_tx,
    })
}

// pub async fn setup_vote_out_escrow_txs(
//     algod: &Algod,
//     creator: Address,
//     shares_asset_id: u64,
//     votes_asset_id: u64,
// ) -> Result<SetupVoteOutEscrowToSign> {
//     todo!()
//     // let escrow = create_vote_out_escrow(algod, shares_asset_id, votes_asset_id).await?;
//     // println!("Generated votes out escrow address: {:?}", escrow.address);

//     // let params = algod.suggested_transaction_params().await?;

//     // // Send some funds to the escrow (min amount to hold asset, pay for opt in tx fee)
//     // let fund_algos_tx = TxnBuilder::with(
//     //     params.clone(),
//     //     Pay::new(creator, escrow.address, MicroAlgos(1_000_000)).build(),
//     // )
//     // .build();

//     // // Escrow opts in to the vote asset
//     // let votes_optin_tx = TxnBuilder::with(
//     //     params.clone(),
//     //     AcceptAsset::new(escrow.address, votes_asset_id).build(),
//     // )
//     // .build();
//     // // let votes_optin_signed_tx = escrow.sign(votes_optin_tx, vec![])?;

//     // // TODO is it possible and does it make sense to execute these atomically?,
//     // // "sc opts in to asset and I send funds to sc"
//     // // TxGroup::assign_group_id(vec![optin_tx, fund_tx])?;

//     // Ok(SetupVoteOutEscrowToSign {
//     //     escrow,
//     //     escrow_votes_optin_tx: votes_optin_tx,
//     //     escrow_funding_algos_tx: fund_algos_tx,
//     // })
// }

async fn create_vote_out_escrow(
    algod: &Algod,
    source: TealSourceTemplate,
    shares_asset_id: u64,
    votes_asset_id: u64,
    staking_escrow_address: Address,
) -> Result<ContractAccount> {
    let escrow = load_vote_out_escrow(
        algod,
        source,
        shares_asset_id,
        votes_asset_id,
        staking_escrow_address,
    )
    .await?;
    Ok(ContractAccount::new(escrow))
}

async fn load_vote_out_escrow(
    algod: &Algod,
    source: TealSourceTemplate,
    shares_asset_id: u64,
    votes_asset_id: u64,
    staking_escrow_address: Address,
) -> Result<CompiledTeal> {
    let source = render_vote_out_escrow(
        source,
        shares_asset_id,
        votes_asset_id,
        staking_escrow_address,
    )?;
    Ok(algod.compile_teal(&source.0).await?)
}

fn render_vote_out_escrow(
    source: TealSourceTemplate,
    shares_asset_id: u64,
    votes_asset_id: u64,
    staking_escrow_address: Address,
) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        VotesOutEscrowTemplateContext {
            shares_asset_id: shares_asset_id.to_string(),
            votes_asset_id: votes_asset_id.to_string(),
            staking_escrow_address: staking_escrow_address.to_string(),
        },
    )?;
    // save_rendered_teal(file_name, escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

#[derive(Serialize)]
struct VotesOutEscrowTemplateContext {
    shares_asset_id: String,
    votes_asset_id: String,
    staking_escrow_address: String,
}

pub async fn submit_vote_out_setup_escrow(
    algod: &Algod,
    signed: SetupVoteOutEscrowSigned,
) -> Result<SubmitVoteOutSetupEscrowRes> {
    // TODO why not grouped? (also in the votein file probably?)
    let fund_escrow_algos_res = algod
        .broadcast_signed_transaction(&signed.funding_algos_tx)
        .await?;
    println!("fund_escrow_algos_res: {:?}", fund_escrow_algos_res);

    let votes_optin_escrow_res = algod
        .broadcast_signed_transaction(&signed.votes_optin_tx)
        .await?;
    println!("votes_optin_escrow_res: {:?}", votes_optin_escrow_res);

    Ok(SubmitVoteOutSetupEscrowRes {
        fund_escrow_algos_tx_id: fund_escrow_algos_res.tx_id,
        votes_optin_escrow_algos_tx_id: votes_optin_escrow_res.tx_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupVoteOutEscrowToSign {
    pub escrow: ContractAccount,
    pub escrow_votes_optin_tx: Transaction,
    pub escrow_funding_algos_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupVoteOutEscrowSigned {
    pub escrow: ContractAccount,
    pub votes_optin_tx: SignedTransaction,
    pub funding_algos_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitVoteOutSetupEscrowRes {
    pub fund_escrow_algos_tx_id: String,
    pub votes_optin_escrow_algos_tx_id: String,
}

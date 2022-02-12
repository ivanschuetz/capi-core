use super::load_project::{ProjectId, TxId};
use crate::flows::{create_project::model::Project, stake::stake::stake_shares_tx};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;

pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

/// Investing transactions specific to the creator (e.g. creator doesn't need to opt-in to the asset)
pub async fn creator_investor_setup(
    algod: &Algod,
    creator: &Address,
    app_id: u64,
    shares_id: u64,
    project_id: &ProjectId,
    project: &Project,
) -> Result<CreatorInvestorSetupToSign> {
    let params = algod.suggested_transaction_params().await?;

    let mut investor_app_setup_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(*creator, app_id)
            .foreign_assets(vec![shares_id])
            .app_arguments(vec![project_id.bytes().to_vec()])
            .build(),
    )
    .build();

    let mut stake_shares_tx = stake_shares_tx(
        &params,
        creator,
        project.shares_asset_id,
        project.specs.creator_part(),
        project.staking_escrow.address(),
    );

    TxGroup::assign_group_id(vec![&mut investor_app_setup_tx, &mut stake_shares_tx])?;

    Ok(CreatorInvestorSetupToSign {
        investor_app_setup_tx,
        stake_shares_tx,
    })
}

pub async fn submit_creator_investor_setup(
    algod: &Algod,
    signed: CreatorInvestorSetupSigned,
) -> Result<TxId> {
    let txs = vec![signed.investor_app_setup_tx, signed.stake_shares_tx];

    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Creator investor setup tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatorInvestorSetupToSign {
    pub investor_app_setup_tx: Transaction,
    pub stake_shares_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatorInvestorSetupSigned {
    pub investor_app_setup_tx: SignedTransaction,
    pub stake_shares_tx: SignedTransaction,
}

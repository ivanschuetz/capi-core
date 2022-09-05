use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::models::{dao_app_id::DaoAppId, tx_id::TxId};
use serde::{Deserialize, Serialize};

#[allow(clippy::too_many_arguments)]
pub async fn team(
    algod: &Algod,
    sender: &Address,
    app_id: DaoAppId,
    url: &str,
) -> Result<SetTeamToSign> {
    log::debug!("Will create team tx with url: {url:?}");

    let params = algod.suggested_transaction_params().await?;

    let app_call_tx = team_app_call(app_id, &params, sender, url)?;

    Ok(SetTeamToSign { app_call_tx })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub uuid: String,
    pub name: String,
    pub descr: String,
    pub role: String,
    pub picture: String,
    pub social_links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub members: Vec<TeamMember>,
}

pub fn team_app_call(
    app_id: DaoAppId,
    params: &SuggestedTransactionParams,
    sender: &Address,
    team_url: &str,
) -> Result<Transaction> {
    log::debug!("??? team url: {team_url:?}");
    log::debug!("??? team url bytes: {:?}", team_url.as_bytes().len());
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*sender, app_id.0)
            .app_arguments(vec![
                "team".as_bytes().to_vec(),
                team_url.as_bytes().to_vec(),
            ])
            .build(),
    )
    .build()?;
    Ok(tx)
}

pub async fn submit_team(algod: &Algod, signed: &SetTeamSigned) -> Result<TxId> {
    log::debug!("calling submit team..");

    let res = algod
        .broadcast_signed_transactions(&[signed.app_call_tx.clone()])
        .await?;
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTeamToSign {
    pub app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTeamSigned {
    pub app_call_tx: SignedTransaction,
}

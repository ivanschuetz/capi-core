use algonaut::{
    algod::v2::Algod, core::Address, indexer::v2::Indexer,
    model::indexer::v2::QueryAccountTransaction,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    api::api::Api,
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    date_util::timestamp_seconds_to_date,
    flows::{
        create_dao::storage::load_dao::{load_dao, DaoId, TxId},
        withdraw::note::base64_withdrawal_note_to_withdrawal_description,
    },
    funds::FundsAmount,
};
use anyhow::{anyhow, Error, Result};

pub async fn withdrawals(
    algod: &Algod,
    indexer: &Indexer,
    creator: &Address,
    dao_id: DaoId,
    api: &dyn Api,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<Withdrawal>> {
    log::debug!("Querying withdrawals by: {:?}", creator);

    let dao = load_dao(algod, dao_id, api, capi_deps).await?;

    let query = QueryAccountTransaction {
        // For now no prefix filtering
        // Algorand's indexer has performance problems with note-prefix and it doesn't work at all with AlgoExplorer or PureStake currently:
        // https://github.com/algorand/indexer/issues/358
        // https://github.com/algorand/indexer/issues/669
        // note_prefix: Some(withdraw_note_prefix_base64()),
        ..Default::default()
    };

    // TODO filter txs by receiver (creator) - this returns everything associated with creator
    let txs = indexer
        .account_transactions(creator, &query)
        .await?
        .transactions;

    // TODO (low prio) compare performance of above vs this (i.e. querying account txs vs txs with receiver field)
    // Note that none is using note prefix currently, see note in query above.
    // let query = QueryTransaction {
    //     address: Some(creator.to_string()),
    //     address_role: Some(Role::Receiver),
    //     ..Default::default()
    // };
    // let txs = indexer.transactions(&query).await?.transactions;

    let mut withdrawals = vec![];

    for tx in &txs {
        // withdrawals are payments - ignore other txs
        if let Some(payment) = tx.asset_transfer_transaction.clone() {
            let sender_address = tx.sender.parse::<Address>().map_err(Error::msg)?;
            let receiver_address = payment.receiver.parse::<Address>().map_err(Error::msg)?;

            // account_transactions returns all the txs "related" to the account, i.e. can be sender or receiver
            // we're interested only in central escrow -> creator
            if sender_address == *dao.central_escrow.address() && receiver_address == *creator {
                // for now the only payload is the description
                let withdrawal_description = match &tx.note {
                    Some(note) => base64_withdrawal_note_to_withdrawal_description(note)?,
                    None => "".to_owned(),
                };

                // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
                // Unclear when it's None. For now we just reject it.
                let round_time = tx
                    .round_time
                    .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

                withdrawals.push(Withdrawal {
                    amount: FundsAmount::new(payment.amount),
                    description: withdrawal_description,
                    date: timestamp_seconds_to_date(round_time)?,
                    tx_id: tx.id.clone().parse()?,
                })
            }
        }
    }

    Ok(withdrawals)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Withdrawal {
    pub amount: FundsAmount,
    pub description: String,
    pub date: DateTime<Utc>,
    pub tx_id: TxId,
}

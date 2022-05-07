use algonaut::{
    algod::v2::Algod,
    core::Address,
    indexer::v2::Indexer,
    model::indexer::v2::{QueryAccountTransaction, TransactionType},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    api::teal_api::TealApi,
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    date_util::timestamp_seconds_to_date,
    flows::{
        create_dao::storage::load_dao::{load_dao, DaoId, TxId},
        withdraw::note::base64_withdrawal_note_to_withdrawal_description,
    },
    funds::{FundsAmount, FundsAssetId},
};
use anyhow::{anyhow, Error, Result};

pub async fn withdrawals(
    algod: &Algod,
    indexer: &Indexer,
    owner: &Address,
    dao_id: DaoId,
    api: &dyn TealApi,
    funds_asset: FundsAssetId,
    capi_deps: &CapiAssetDaoDeps,
    after_time: &Option<DateTime<Utc>>,
) -> Result<Vec<Withdrawal>> {
    log::debug!("Querying withdrawals by: {:?}", owner);

    let dao = load_dao(algod, dao_id, api, capi_deps).await?;

    let after_time_formatted = after_time.map(|t| t.to_rfc3339());

    let query = QueryAccountTransaction {
        tx_type: Some(TransactionType::ApplicationTransaction),
        after_time: after_time_formatted,
        // For now no prefix filtering
        // Algorand's indexer has performance problems with note-prefix and it doesn't work at all with AlgoExplorer or PureStake currently:
        // https://github.com/algorand/indexer/issues/358
        // https://github.com/algorand/indexer/issues/669
        // note_prefix: Some(withdraw_note_prefix_base64()),
        ..Default::default()
    };

    // TODO filter txs by receiver (creator) - this returns everything associated with creator
    let txs = indexer
        .account_transactions(owner, &query)
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
        if let Some(app_call) = tx.application_transaction.clone() {
            if app_call.application_id == dao.app_id.0 {
                for inner_tx in &tx.inner_txns {
                    // withdrawals are xfers from the app to the owner
                    if let Some(xfer) = inner_tx.asset_transfer_transaction.clone() {
                        // not sure that we need to check the sender here - it's probably always the app? but it doesn't hurt
                        let sender_address =
                            inner_tx.sender.parse::<Address>().map_err(Error::msg)?;
                        let receiver_address =
                            xfer.receiver.parse::<Address>().map_err(Error::msg)?;

                        // account_transactions returns all the txs "related" to the account, i.e. can be sender or receiver
                        // we're interested only in central escrow -> creator
                        if FundsAssetId(xfer.asset_id) == funds_asset
                            && sender_address == dao.app_address()
                            && receiver_address == *owner
                        {
                            // for now the only payload is the description
                            let withdrawal_description = match &tx.note {
                                Some(note) => {
                                    base64_withdrawal_note_to_withdrawal_description(note)?
                                }
                                None => "".to_owned(),
                            };

                            // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
                            // Unclear when it's None. For now we just reject it.
                            let round_time = tx.round_time.ok_or_else(|| {
                                anyhow!("Unexpected: tx has no round time: {:?}", tx)
                            })?;

                            let id = tx
                                .id
                                .clone()
                                .ok_or_else(|| anyhow!("Unexpected: tx has no id: {:?}", tx))?;

                            withdrawals.push(Withdrawal {
                                amount: FundsAmount::new(xfer.amount),
                                description: withdrawal_description,
                                date: timestamp_seconds_to_date(round_time)?,
                                tx_id: id.parse()?,
                            })
                        }
                    }
                }
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

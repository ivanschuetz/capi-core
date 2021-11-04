use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    model::algod::v2::{Account, ApplicationLocalState},
};
use anyhow::Result;

use super::app_state::{
    global_state, local_state, local_state_from_account, AppStateKey, ApplicationLocalStateError,
    ApplicationStateExt,
};

const GLOBAL_AMOUNT: AppStateKey = AppStateKey("Amount");
const GLOBAL_VOTES: AppStateKey = AppStateKey("Votes");
const GLOBAL_WITHDRAWAL_AMOUNT: AppStateKey = AppStateKey("WRound");

const LOCAL_VOTES: AppStateKey = AppStateKey("LVotes");
const LOCAL_VALID: AppStateKey = AppStateKey("Valid");
const LOCAL_VOTED_ROUND: AppStateKey = AppStateKey("VWRound");

pub struct WithdrawalSlotGlobalState {
    pub amount: MicroAlgos,
    pub votes: u64,
    pub withdrawal_round: u64,
}

impl WithdrawalSlotGlobalState {
    pub fn has_active_request(&self) -> bool {
        self.amount.0 > 0
    }

    // consider leaving only has_active_request
    pub fn is_free(&self) -> bool {
        !self.has_active_request()
    }
}

pub async fn withdrawal_slot_global_state(
    algod: &Algod,
    app_id: u64,
) -> Result<WithdrawalSlotGlobalState> {
    let global_state = global_state(algod, app_id).await?;
    let amount = MicroAlgos(global_state.find_uint(&GLOBAL_AMOUNT).unwrap_or(0));
    let votes = global_state.find_uint(&GLOBAL_VOTES).unwrap_or(0);
    let withdrawal_round = global_state
        .find_uint(&GLOBAL_WITHDRAWAL_AMOUNT)
        .unwrap_or(0);

    Ok(WithdrawalSlotGlobalState {
        amount,
        votes,
        withdrawal_round,
    })
}

pub struct WithdrawalSlotVoterState {
    pub votes: u64,
    pub valid: bool,
    pub voted_round: u64,
}

impl WithdrawalSlotVoterState {
    pub fn did_vote_in_current_round(&self) -> bool {
        // votes > 0 are always for current round,
        // they're set to 0 when initializing a new voting round (by project creator)
        self.votes > 0
    }
}

pub async fn withdrawal_slot_voter_state(
    algod: &Algod,
    voter: &Address,
    app_id: u64,
) -> Result<WithdrawalSlotVoterState, ApplicationLocalStateError> {
    let local_state = local_state(algod, voter, app_id).await?;
    withdrawal_slot_voter_state_with_local_state(local_state)
}

pub fn withdrawal_slot_voter_state_with_account(
    account: &Account,
    app_id: u64,
) -> Result<WithdrawalSlotVoterState, ApplicationLocalStateError> {
    let local_state = local_state_from_account(account, app_id)?;
    withdrawal_slot_voter_state_with_local_state(local_state)
}

fn withdrawal_slot_voter_state_with_local_state(
    state: ApplicationLocalState,
) -> Result<WithdrawalSlotVoterState, ApplicationLocalStateError> {
    let votes = state.find_uint(&LOCAL_VOTES).unwrap_or(0);
    let valid_u64 = state.find_uint(&LOCAL_VALID).unwrap_or(0);
    let voted_round = state.find_uint(&LOCAL_VOTED_ROUND).unwrap_or(0);

    let valid = match valid_u64 {
        0 => false,
        1 => true,
        _ => {
            return Err(ApplicationLocalStateError::Msg(format!(
                "Invalid valid value: {}",
                valid_u64
            )))
        }
    };

    Ok(WithdrawalSlotVoterState {
        votes,
        valid,
        voted_round,
    })
}

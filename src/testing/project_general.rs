#[cfg(test)]
use crate::state::withdrawal_app_state::withdrawal_slot_voter_state;
#[cfg(test)]
use algonaut::algod::v2::Algod;
#[cfg(test)]
use algonaut::core::Address;
#[cfg(test)]
use algonaut::model::algod::v2::Application;
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub fn check_schema(app: &Application) {
    assert!(app.params.global_state_schema.is_some());
    let app_global_state_schema = app.params.global_state_schema.as_ref().unwrap();
    assert_eq!(0, app_global_state_schema.num_byte_slice);
    assert_eq!(1, app_global_state_schema.num_uint);
    assert!(app.params.local_state_schema.is_some());
    let app_local_state_schema = app.params.local_state_schema.as_ref().unwrap();
    assert_eq!(0, app_local_state_schema.num_byte_slice);
    assert_eq!(2, app_local_state_schema.num_uint);
}

#[cfg(test)]
pub async fn test_withdrawal_slot_local_state_initialized_correctly(
    algod: &Algod,
    investor_address: &Address,
    app_id: u64,
) -> Result<()> {
    let investor_state = withdrawal_slot_voter_state(algod, investor_address, app_id).await?;
    assert_eq!(0, investor_state.votes);
    assert_eq!(true, investor_state.valid);
    assert_eq!(0, investor_state.voted_round);
    Ok(())
}

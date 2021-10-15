use algonaut::model::algod::v2::Application;
use data_encoding::BASE64;

pub fn withdrawal_amount_global_state(apps_state: &Application) -> Option<u64> {
    apps_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"Amount"))
        .map(|s| s.value.uint)
}

pub fn votes_global_state(apps_state: &Application) -> Option<u64> {
    apps_state
        .params
        .global_state
        .iter()
        .find(|s| s.key == BASE64.encode(b"Votes"))
        .map(|s| s.value.uint)
}

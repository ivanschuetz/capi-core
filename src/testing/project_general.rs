#[cfg(test)]
use algonaut::model::algod::v2::Application;

// Leaving this there - might make sense to test this, when testing is more detailed.
#[allow(dead_code)]
#[cfg(test)]
pub fn check_schema(app: &Application) {
    assert!(app.params.global_state_schema.is_some());
    let app_global_state_schema = app.params.global_state_schema.as_ref().unwrap();
    assert_eq!(0, app_global_state_schema.num_byte_slice);
    assert_eq!(1, app_global_state_schema.num_uint);
    assert!(app.params.local_state_schema.is_some());
    let app_local_state_schema = app.params.local_state_schema.as_ref().unwrap();
    assert_eq!(1, app_local_state_schema.num_byte_slice);
    assert_eq!(2, app_local_state_schema.num_uint);
}

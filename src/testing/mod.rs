#[cfg(test)]
pub mod algorand_checks;
pub mod create_and_submit_txs;
pub mod flow;
pub mod generate_mnemonic;
pub mod network_test_util;
pub mod project_general;
pub mod test_data;

#[cfg(test)]
pub const TESTS_DEFAULT_PRECISION: u64 = 10_000;

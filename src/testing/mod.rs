#[cfg(test)]
pub mod algorand_checks;
pub mod create_and_submit_txs;
pub mod dao_general;
pub mod flow;
pub mod generate_mnemonic;
pub mod network_test_util;
pub mod test_data;
pub mod tests_msig;

#[cfg(test)]
pub const TESTS_DEFAULT_PRECISION: u64 = 10_000;

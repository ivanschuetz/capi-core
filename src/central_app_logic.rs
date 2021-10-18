use algonaut::core::MicroAlgos;

pub fn calculate_entitled_harvest(
    central_total_received: MicroAlgos,
    share_supply: u64,
    owned_shares: u64,
) -> MicroAlgos {
    // the % the investor is entitled to
    // TODO review these maths (e.g. floor)
    // also use Decimal lib
    // test division result with fractional digits
    let investor2_entitled_dividends = owned_shares as f64 / share_supply as f64;
    MicroAlgos((central_total_received.0 as f64 * investor2_entitled_dividends).floor() as u64)
}

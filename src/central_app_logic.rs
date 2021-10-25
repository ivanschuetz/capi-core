use algonaut::core::MicroAlgos;
use rust_decimal::prelude::ToPrimitive;

use crate::decimal_util::AsDecimal;

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

pub fn investor_can_harvest_amount_calc(
    central_received_total: MicroAlgos,
    harvested_total: MicroAlgos,
    share_count: u64,
    share_supply: u64,
    precision: u64,
) -> MicroAlgos {
    // TODO review possible overflow, type cast
    // for easier understanding we use the same arithmetic as in TEAL
    let entitled_percentage =
        ((share_count * precision).as_decimal() / share_supply.as_decimal()).floor();
    let entitled_total = ((central_received_total.0.as_decimal() * entitled_percentage)
        / precision.as_decimal())
    .floor();

    // Note that this assumes that investor can't unstake only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with harvested_total, which remains unchanged.
    // the easiest solution is to expect the investor to unstake all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.
    // unwrap: floor

    MicroAlgos(entitled_total.to_u128().unwrap() as u64) - harvested_total
}

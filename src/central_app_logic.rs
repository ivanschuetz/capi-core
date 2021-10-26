use algonaut::core::MicroAlgos;
use rust_decimal::prelude::ToPrimitive;

use crate::decimal_util::AsDecimal;

pub fn calculate_entitled_harvest(
    central_received_total: MicroAlgos,
    share_supply: u64,
    share_count: u64,
    precision: u64,
    investors_share: u64,
) -> MicroAlgos {
    // TODO review possible overflow, type cast, unwrap
    // for easier understanding we use the same arithmetic as in TEAL
    let investors_share_fractional_percentage = investors_share.as_decimal() / 100.as_decimal(); // e.g. 10% -> 0.1

    let entitled_percentage = ((share_count * precision).as_decimal()
        * (investors_share_fractional_percentage * precision.as_decimal())
        / share_supply.as_decimal())
    .floor();

    let entitled_total = ((central_received_total.0.as_decimal() * entitled_percentage)
        / (precision.as_decimal() * precision.as_decimal()))
    .floor();

    MicroAlgos(entitled_total.to_u128().unwrap() as u64)
}

pub fn investor_can_harvest_amount_calc(
    central_received_total: MicroAlgos,
    harvested_total: MicroAlgos,
    share_count: u64,
    share_supply: u64,
    precision: u64,
    investors_share: u64,
) -> MicroAlgos {
    // Note that this assumes that investor can't unstake only a part of their shares
    // otherwise, the smaller share count would render a small entitled_total_count which would take a while to catch up with harvested_total, which remains unchanged.
    // the easiest solution is to expect the investor to unstake all their shares
    // if they want to sell only a part, they've to opt-in again with the shares they want to keep.

    let entitled_total = calculate_entitled_harvest(
        central_received_total,
        share_supply,
        share_count,
        precision,
        investors_share,
    );
    entitled_total - harvested_total
}

#[cfg(test)]
use crate::flows::create_project::{
    create_project_specs::CreateProjectSpecs, create_shares_specs::CreateSharesSpecs,
};
#[cfg(test)]
use algonaut::core::MicroAlgos;
#[cfg(test)]
use algonaut::transaction::account::Account;

#[cfg(test)]
pub fn creator() -> Account {
    // VKCFMGBTVINZ4EN7253QVTALGYQRVMOLVHF6O44O2X7URQP7BAOAXXPFCA
    Account::from_mnemonic("town clutch grain accident sheriff wagon meadow shaft saddle door all town supply indicate deliver about arrange hire kit curve destroy gloom attitude absorb excite").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn investor1() -> Account {
    // WZOKN67NQUMY5ZV7Q2KOBKUY5YP3L5UFFOWBUV6HKXKFMLCUWTNZJRSI4E
    Account::from_mnemonic("phone similar album unusual notable initial evoke party garlic gain west catch bike enforce layer bring suggest shiver script venue couple tooth special abandon ranch").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn investor2() -> Account {
    // ZRPA4PEHLXIT4WWEKXFJMWF4FNBCA4P4AYC36H7VGNSINOJXWSQZB2XCP4
    Account::from_mnemonic("abandon include valid approve among begin disorder hint option train palace drink enable enter shallow various bid jacket record left derive memory magnet able phrase").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn customer() -> Account {
    // added to sandbox (script)
    Account::from_mnemonic("clog coral speak since defy siege video lamp polar chronic treat smooth puzzle input payment hobby draft habit race birth ridge correct behave able close").unwrap()
}

#[cfg(test)]
pub fn project_specs() -> CreateProjectSpecs {
    // use rust_decimal::Decimal;

    // let percentage = 40;
    // let shares_specs = shares_specs();
    // let percentage_decimal = Decimal::from(percentage) / Decimal::from(100);

    CreateProjectSpecs::new(
        "Pancakes ltd".to_owned(),
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat".to_owned(),
        shares_specs(),
        40,
        MicroAlgos(5_000_000),
        // shares_distribution: SharesDistributionSpecs::from_investors_percentage(percentage_decimal.try_into().unwrap(), shares_specs.count).unwrap(),
        "https://placekitten.com/200/300".to_string(),
        "https://twitter.com/capi_fin".to_owned(),
    // unwrap: error only if investors_count > count and we're passing correct hardcoded values + this is only for testing
    ).unwrap()
}

#[cfg(test)]
pub fn shares_specs() -> CreateSharesSpecs {
    CreateSharesSpecs {
        token_name: "PCK".to_owned(),
        count: 100,
    }
}

#[cfg(test)]
use crate::flows::create_project::share_amount::ShareAmount;
#[cfg(test)]
use crate::flows::create_project::{
    create_project_specs::CreateProjectSpecs, model::CreateSharesSpecs,
};
#[cfg(test)]
use crate::funds::FundsAmount;
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

#[allow(dead_code)]
#[cfg(test)]
pub fn capi_owner() -> Account {
    Account::from_mnemonic("champion slab oyster plug add neutral gap burger civil gossip hybrid return truth mad light edit invest hybrid mistake allow flip quarter guess abstract ginger").unwrap()
}

#[cfg(test)]
pub fn project_specs() -> CreateProjectSpecs {
    CreateProjectSpecs::new(
        "Pancakes ltd".to_owned(),
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat".to_owned(),
        shares_specs(),
        ShareAmount(40),
        FundsAmount(5_000_000),
        "https://placekitten.com/200/300".to_string(),
        "https://twitter.com/capi_fin".to_owned(),
    // unwrap: hardcoded (test) data, we know it's correct
    ).unwrap()
}

#[cfg(test)]
pub fn shares_specs() -> CreateSharesSpecs {
    CreateSharesSpecs {
        token_name: "PCK".to_owned(),
        supply: ShareAmount(100),
    }
}

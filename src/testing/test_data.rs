#[cfg(test)]
use crate::flows::create_project::model::{CreateProjectSpecs, CreateSharesSpecs};
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
    CreateProjectSpecs {
        name: "Pancakes ltd".to_owned(),
        shares: shares_specs(),
        asset_price: MicroAlgos(5_000_000),
        vote_threshold: 70,
        investors_share: 40,
    }
}

#[cfg(test)]
pub fn shares_specs() -> CreateSharesSpecs {
    CreateSharesSpecs {
        token_name: "PCK".to_owned(),
        count: 100,
    }
}

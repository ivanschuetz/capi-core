#[cfg(test)]
use crate::flows::create_project::model::{CreateProjectSpecs, CreateSharesSpecs};
#[cfg(test)]
use algonaut::core::MicroAlgos;
#[cfg(test)]
use algonaut::transaction::account::Account;

#[cfg(test)]
pub fn creator() -> Account {
    Account::from_mnemonic("fire enlist diesel stamp nuclear chunk student stumble call snow flock brush example slab guide choice option recall south kangaroo hundred matrix school above zero").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn investor1() -> Account {
    Account::from_mnemonic("since during average anxiety protect cherry club long lawsuit loan expand embark forum theory winter park twenty ball kangaroo cram burst board host ability left").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn investor2() -> Account {
    Account::from_mnemonic("auction inquiry lava second expand liberty glass involve ginger illness length room item discover ahead table doctor term tackle cement bonus profit right above catch").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn customer() -> Account {
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

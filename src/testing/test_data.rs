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
    // STOUDMINSIPP7JMJMGXVJYVS6HHD3TT5UODCDPYGV6KBGP7UYNTLJVJJME
    Account::from_mnemonic("frame engage radio switch little scan time column amused spatial dynamic play cruise split coral aisle midnight cave essence midnight mutual dog ostrich absent leopard").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn investor1() -> Account {
    // 7XSZQUQ2GJB25W37LVM5R4CMKKVC4VNSMIPCIWJYWM5ORA5VA4JRCNOJ4Y
    Account::from_mnemonic("wood purse siege pencil silk ladder hedgehog aim bulk enlist crisp abuse patch direct oval cool parent tail debris zoo youth false suit absorb prefer").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn investor2() -> Account {
    // PGCS3D5JL4AIFGTBPDGGMMCT3ODKUUFEFG336MJO25CGBG7ORKVOE3AHSU
    Account::from_mnemonic("general assist twist drill snake height piano stamp lazy room firm link because link charge flight rail join prosper area oppose license mercy abstract cherry").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn customer() -> Account {
    // 7ZLNWP5YP5DCCCLHAYYETZQLFH4GTBEKTBFQDHA723I7BBZ2FKCOZCBE4I
    // added to sandbox (script)
    Account::from_mnemonic("group slush snack cram emotion echo cousin viable fan all pupil solar total boss deny under master rely wine help trick mechanic glance abstract clever").unwrap()
}

#[allow(dead_code)]
#[cfg(test)]
pub fn capi_owner() -> Account {
    // NIKGABIQLRCPJYCNCFZWR7GUIC3NA66EBVR65JKHKLGLIYQ4KO3YYPV67Q
    Account::from_mnemonic("accident inherit artist kid such wheat sure then skirt horse afford penalty grant airport school aim hollow position ask churn extend soft mean absorb achieve").unwrap()
}

#[test]
fn print_addresses() {
    println!("creator: {}", creator().address());
    println!("investor1: {}", investor1().address());
    println!("investor2: {}", investor2().address());
    println!("customer: {}", customer().address());
    println!("capi_owner: {}", capi_owner().address());
}

#[cfg(test)]
pub fn project_specs() -> CreateProjectSpecs {
    CreateProjectSpecs::new(
        "Pancakes ltd".to_owned(),
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat".to_owned(),
        shares_specs(),
        ShareAmount::new(40),
        FundsAmount::new(5_000_000),
        "https://placekitten.com/200/300".to_string(),
        "https://twitter.com/capi_fin".to_owned(),
    // unwrap: hardcoded (test) data, we know it's correct
    ).unwrap()
}

#[cfg(test)]
pub fn shares_specs() -> CreateSharesSpecs {
    CreateSharesSpecs {
        token_name: "PCK".to_owned(),
        supply: ShareAmount::new(100),
    }
}

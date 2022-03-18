#[cfg(test)]
pub use test::{
    capi_owner, creator, customer, dao_specs, investor1, investor2, msig_acc1, msig_acc2,
    msig_acc3, shares_specs,
};

#[cfg(test)]
mod test {
    use crate::flows::create_dao::share_amount::ShareAmount;
    use crate::flows::create_dao::{create_dao_specs::CreateDaoSpecs, model::CreateSharesSpecs};
    use crate::funds::FundsAmount;
    use algonaut::transaction::account::Account;

    pub fn creator() -> Account {
        // STOUDMINSIPP7JMJMGXVJYVS6HHD3TT5UODCDPYGV6KBGP7UYNTLJVJJME
        Account::from_mnemonic("frame engage radio switch little scan time column amused spatial dynamic play cruise split coral aisle midnight cave essence midnight mutual dog ostrich absent leopard").unwrap()
    }

    pub fn investor1() -> Account {
        // 7XSZQUQ2GJB25W37LVM5R4CMKKVC4VNSMIPCIWJYWM5ORA5VA4JRCNOJ4Y
        Account::from_mnemonic("wood purse siege pencil silk ladder hedgehog aim bulk enlist crisp abuse patch direct oval cool parent tail debris zoo youth false suit absorb prefer").unwrap()
    }

    pub fn investor2() -> Account {
        // PGCS3D5JL4AIFGTBPDGGMMCT3ODKUUFEFG336MJO25CGBG7ORKVOE3AHSU
        Account::from_mnemonic("general assist twist drill snake height piano stamp lazy room firm link because link charge flight rail join prosper area oppose license mercy abstract cherry").unwrap()
    }

    pub fn customer() -> Account {
        // 7ZLNWP5YP5DCCCLHAYYETZQLFH4GTBEKTBFQDHA723I7BBZ2FKCOZCBE4I
        // added to sandbox (script)
        Account::from_mnemonic("group slush snack cram emotion echo cousin viable fan all pupil solar total boss deny under master rely wine help trick mechanic glance abstract clever").unwrap()
    }

    pub fn capi_owner() -> Account {
        // NIKGABIQLRCPJYCNCFZWR7GUIC3NA66EBVR65JKHKLGLIYQ4KO3YYPV67Q
        Account::from_mnemonic("accident inherit artist kid such wheat sure then skirt horse afford penalty grant airport school aim hollow position ask churn extend soft mean absorb achieve").unwrap()
    }

    pub fn msig_acc1() -> Account {
        // DN7MBMCL5JQ3PFUQS7TMX5AH4EEKOBJVDUF4TCV6WERATKFLQF4MQUPZTA
        Account::from_mnemonic("auction inquiry lava second expand liberty glass involve ginger illness length room item discover ahead table doctor term tackle cement bonus profit right above catch").unwrap()
    }

    pub fn msig_acc2() -> Account {
        // GIZTTA56FAJNAN7ACK3T6YG34FH32ETDULBZ6ENC4UV7EEHPXJGGSPCMVU
        Account::from_mnemonic("fire enlist diesel stamp nuclear chunk student stumble call snow flock brush example slab guide choice option recall south kangaroo hundred matrix school above zero").unwrap()
    }

    pub fn msig_acc3() -> Account {
        // BFRTECKTOOE7A5LHCF3TTEOH2A7BW46IYT2SX5VP6ANKEXHZYJY77SJTVM
        Account::from_mnemonic("since during average anxiety protect cherry club long lawsuit loan expand embark forum theory winter park twenty ball kangaroo cram burst board host ability left").unwrap()
    }

    #[test]
    fn print_addresses() {
        println!("creator: {}", creator().address());
        println!("investor1: {}", investor1().address());
        println!("investor2: {}", investor2().address());
        println!("customer: {}", customer().address());
        println!("capi_owner: {}", capi_owner().address());
        println!("msig1: {}", msig_acc1().address());
        println!("msig2: {}", msig_acc2().address());
        println!("msig3: {}", msig_acc3().address());
    }

    pub fn dao_specs() -> CreateDaoSpecs {
        CreateDaoSpecs::new(
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

    pub fn shares_specs() -> CreateSharesSpecs {
        CreateSharesSpecs {
            token_name: "PCK".to_owned(),
            supply: ShareAmount::new(100),
        }
    }
}

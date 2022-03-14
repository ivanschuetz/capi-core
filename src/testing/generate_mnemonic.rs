#[test]
#[ignore]
fn generate_random_test_accounts_entry() {
    let account = algonaut::transaction::account::Account::generate();
    println!("# {}", account.address()); // address only for info - the # is to ignore when parsing the accounts
    println!("{}", account.mnemonic());
}

#[test]
#[ignore]
fn generate_random_accounts() {
    for _ in 0..100 {
        generate_random_test_accounts_entry()
    }
}

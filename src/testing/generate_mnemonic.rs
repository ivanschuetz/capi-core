#[test]
#[ignore]
fn generate_mnemonic() {
    println!(
        "{}",
        algonaut::transaction::account::Account::generate().mnemonic()
    );
}

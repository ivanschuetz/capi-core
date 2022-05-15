#[cfg(test)]
pub use test::TestsMsig;

#[cfg(test)]
pub mod test {

    use algonaut::{
        core::{Address, MultisigAddress},
        transaction::{
            account::Account, transaction::TransactionSignature, SignedTransaction, Transaction,
        },
    };
    use anyhow::{anyhow, Result};

    #[derive(Debug)]
    pub struct TestsMsig {
        accounts: Vec<Account>,
    }

    impl TestsMsig {
        pub fn new(accounts: Vec<Account>) -> Result<TestsMsig> {
            if accounts.len() < 2 {
                // we need 1 account for unwrap(),
                // and another because this testing class is used for "happy path" msig - it doesn't make sense to msig with only 1 account
                return Err(anyhow!(
                    "Trying to intialize tests msig with less than 2 accounts"
                ));
            }
            Ok(TestsMsig { accounts })
        }

        fn first_account(&self) -> Account {
            let account: &Account = self.accounts.first().unwrap();
            // account can't be cloned, so via the mnemonic
            Account::from_mnemonic(&account.mnemonic()).unwrap()
        }

        pub fn address(&self) -> MultisigAddress {
            let addresses: Vec<Address> = self.accounts.iter().map(|a| a.address()).collect();
            MultisigAddress::new(1, 2, &addresses).unwrap()
        }

        pub fn sign(&self, tx: Transaction) -> Result<SignedTransaction> {
            let address = self.address();

            let mut msig = self.first_account().init_transaction_msig(&tx, &address)?;
            for acc in &self.accounts[1..self.accounts.len()] {
                msig = acc.append_to_transaction_msig(&tx, msig)?;
            }
            log::debug!("Multisig signed: {msig:?}");

            Ok(SignedTransaction {
                transaction: tx,
                transaction_id: "".to_owned(),
                sig: TransactionSignature::Multi(msig),
                auth_address: None,
            })
        }

        #[allow(dead_code)]
        pub fn sign_incomplete(&self, tx: Transaction) -> Result<SignedTransaction> {
            let address = self.address();

            let msig = self.first_account().init_transaction_msig(&tx, &address)?;
            log::debug!("Multisig signed (incomplete): {msig:?}");

            Ok(SignedTransaction {
                transaction: tx,
                transaction_id: "".to_owned(),
                sig: TransactionSignature::Multi(msig),
                auth_address: None,
            })
        }
    }
}

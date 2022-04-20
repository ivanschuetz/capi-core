/// Bunch of recurrent developer queries (e.g. show transaction infos) - not used by the app

#[cfg(test)]
mod tests {
    use crate::dependencies;
    use anyhow::Result;
    use tokio::test;

    #[ignore]
    #[test]
    async fn tx_infos() -> Result<()> {
        let indexer = dependencies::indexer_for_tests();

        let infos = indexer
            .transaction_info("JALJLV4VPV73NZHRF6KN2DGRTUAYDFDKQDOO6RYS77X77FOUDH3A")
            .await?;

        println!("infos: {:#?}", infos);

        Ok(())
    }
}

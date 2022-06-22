#[cfg(test)]
mod tests {
    use crate::{
        flows::create_dao::storage::load_dao::load_dao,
        testing::{flow::create_dao_flow::create_dao_flow, network_test_util::test_dao_init},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn loads_dao() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let created_dao = create_dao_flow(&td).await?;

        let loaded_dao = load_dao(algod, created_dao.id()).await?;

        assert_eq!(created_dao, loaded_dao);

        Ok(())
    }
}

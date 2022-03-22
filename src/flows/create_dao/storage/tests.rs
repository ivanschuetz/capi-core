#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::test;

    use crate::{
        flows::create_dao::storage::load_dao::load_dao,
        testing::{flow::create_dao_flow::create_dao_flow, network_test_util::test_dao_init},
    };

    #[test]
    async fn loads_dao() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let create_dao_res = create_dao_flow(&td).await?;

        let stored_dao = load_dao(
            algod,
            create_dao_res.dao_id,
            &td.programs.escrows,
            &td.dao_deps(),
        )
        .await?;

        assert_eq!(create_dao_res.dao, stored_dao.dao);

        Ok(())
    }
}

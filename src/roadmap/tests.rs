#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::Utc;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        roadmap::{
            add_roadmap_item::{
                add_roadmap_item, submit_add_roadmap_item, AddRoadmapItemToSigned,
                RoadmapItemInputs,
            },
            get_roadmap::{get_roadmap, SavedRoadmapItem},
        },
        testing::{flow::create_dao_flow::create_dao_flow, network_test_util::test_dao_init},
    };

    #[test]
    #[serial]
    // For now ignore, as it needs a long delay (> 1 min) to wait for indexing
    // TODO: can we trigger indexing manually?
    #[ignore]
    async fn test_add_roadmap_item() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let dao = create_dao_flow(&td).await?;

        let inputs = RoadmapItemInputs {
            dao_id: dao.id(),
            title: "MVP Release".to_owned(),
            parent: Box::new(None),
            date: Utc::now(),
        };

        let to_sign = add_roadmap_item(&algod, &td.creator.address(), &inputs).await?;

        let signed_tx = td.creator.sign_transaction(to_sign.tx)?;

        let tx_id =
            submit_add_roadmap_item(&algod, &AddRoadmapItemToSigned { tx: signed_tx }).await?;
        log::debug!("Add roadmap item tx id: {}", tx_id);

        log::debug!("Waiting for indexing..");
        std::thread::sleep(std::time::Duration::from_secs(120));

        // check that the item was added correctly

        let saved_roadmap = get_roadmap(&td.indexer, &td.creator.address(), dao.id()).await?;

        assert_eq!(1, saved_roadmap.items.len());

        // check that we can retrieve the same item we just saved
        let saved_item = &saved_roadmap.items[0];
        assert_eq!(
            inputs,
            saved_roadmap_item_into_roadmap_item_inputs(saved_item)
        );

        Ok(())
    }

    // we need this convertion only for tests so here and explicit
    fn saved_roadmap_item_into_roadmap_item_inputs(
        saved_item: &SavedRoadmapItem,
    ) -> RoadmapItemInputs {
        RoadmapItemInputs {
            dao_id: saved_item.dao_id.clone(),
            title: saved_item.title.clone(),
            parent: saved_item.parent.clone(),
            date: saved_item.date.clone(),
        }
    }
}

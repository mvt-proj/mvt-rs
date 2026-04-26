#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::future::join_all;

    async fn mock_get_tile(id: u8) -> Result<(Bytes, String), String> {
        tokio::time::sleep(tokio::time::Duration::from_millis(100 - id as u64)).await;
        Ok((Bytes::from(vec![id]), "Database".to_string()))
    }

    #[tokio::test]
    async fn test_tile_composition_parallelism_and_order() {
        let ids = vec![1, 2, 3];
        let mut futures = Vec::new();

        for id in ids {
            futures.push(mock_get_tile(id));
        }

        let results = join_all(futures).await;

        let mut output_data = Vec::new();
        for result in results {
            if let Ok((tile, _)) = result {
                output_data.push(tile);
            }
        }

        assert_eq!(output_data.len(), 3);

        let final_output = output_data.concat();
        assert_eq!(final_output, vec![1, 2, 3]);
    }
}

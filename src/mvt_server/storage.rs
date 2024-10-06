use mvtrs::common::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage<T> {
    pub file_path: String,
    data: Option<T>,
}

impl<T> Storage<T> {
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            data: None,
        }
    }

    pub async fn save(&mut self, data: T) -> AppResult<()>
    where
        T: Serialize,
    {
        let file_path = Path::new(&self.file_path);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)
            .await?;

        let json = serde_json::to_string(&data)?;
        file.write_all(json.as_bytes()).await?;
        file.flush().await?;

        self.data = Some(data);
        Ok(())
    }

    pub async fn load(&mut self) -> AppResult<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let file_path = Path::new(&self.file_path);
        if !file_path.exists() {
            return Ok(None);
        }

        let mut file = OpenOptions::new().read(true).open(file_path).await?;
        let mut json = String::new();
        file.read_to_string(&mut json).await?;

        let data: T = serde_json::from_str(&json)?;
        self.data = Some(data);
        Ok(self.data.take())
    }

    // pub fn get_data(&mut self) -> Option<T> {
    //     self.data.take()
    // }
}

// #[cfg(test)]
// mod tests {
//     use super::Storage;
//     use tempfile::NamedTempFile;

//     #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
//     struct Person {
//         name: String,
//         age: u32,
//     }
//
//     #[tokio::test]
//     async fn test_save() {
//         let temp_file = NamedTempFile::new().unwrap();
//         let mut storage = Storage::<Person>::new(temp_file.path().to_str().unwrap().to_string());
//
//         let p1 = Person {
//             name: "Toto".to_string(),
//             age: 3,
//         };
//         let _ = storage.save(p1.clone()).await;
//         assert_eq!(storage.get_data().unwrap(), p1);
//     }
//
//     #[tokio::test]
//     async fn test_load() {
//         let temp_file = NamedTempFile::new().unwrap();
//         let mut storage = Storage::<Person>::new(temp_file.path().to_str().unwrap().to_string());
//
//         let p1 = Person {
//             name: "Toto".to_string(),
//             age: 3,
//         };
//         let _ = storage.save(p1.clone()).await;
//         let loaded_data = storage.load().await.unwrap().unwrap();
//         assert_eq!(loaded_data, p1);
//     }
// }

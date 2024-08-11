use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use atomic_refcell::AtomicRefCell;
use meilisearch_sdk::client::Client;
use meilisearch_sdk::search::{SearchResults};
use passwords::PasswordGenerator;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;
use websocket::OwnedMessage;
use crate::blazzy_client::BlazzyData;

#[derive(Serialize, Deserialize, Debug)]
pub struct DataFile {
    id: i32,
    data: BlazzyData
}

pub struct MeilisearchClient {
    exe_path: PathBuf,
    data_dir: PathBuf,
    host: MeilisearchHost,
    master_key: MeilisearchMasterKey,
    client: Option<Client>,
    sender: Option<Sender<OwnedMessage>>,
    recv: Option<Arc<AtomicRefCell<Receiver<OwnedMessage>>>>,
}

impl MeilisearchClient {
    pub(crate) fn get_exe_path(&self) -> &PathBuf {
        &self.exe_path
    }

    pub(crate) fn get_master_key(&self) -> &String {
        &self.master_key.0
    }

    pub(crate) fn get_db_path(&self) -> PathBuf {
        let path = self.data_dir.join("data.ms");
        path
    }

    pub(crate) fn get_dump_dir(&self) -> PathBuf {
        let path = self.data_dir.join("dump/");
        path
    }
}

impl MeilisearchClient {
    pub fn init() -> Self {
        let data_dir = std::env::current_exe().unwrap()
            .parent().unwrap()
            .join("services").join("search_engine");
        let exe_path = data_dir.clone().join("search_engine.exe");
        {
            if !exe_path.exists() {
                let meilisearch_path = exe_path.parent().unwrap();
                if !meilisearch_path.exists() {
                    let services_path = meilisearch_path.parent().unwrap();
                    if !services_path.exists() {
                        std::fs::create_dir(services_path).unwrap();
                    }
                    std::fs::create_dir(meilisearch_path).unwrap();
                }
                let mut file = File::create(&exe_path).unwrap();
                file.write_all(include_bytes!("../../assets/meilisearch-windows-amd64.exe"))
                    .unwrap();
            }
        }
        Self {
            exe_path,
            data_dir,
            host: MeilisearchHost::default(),
            master_key: MeilisearchMasterKey::gen(),
            client: None,
            sender: None,
            recv: None
        }
    }

    pub async fn run(&mut self) -> bool {
        if let Ok(client) = Client::new(
            format!("http://{}:{}", &self.host.0, &self.host.1),
            Some(&self.master_key.0),
        ) {
            self.client = Some(client);
            true
        } else {
            false
        }
    }

    pub async fn create_index(&self, index: &str) {
        if let Some(client) = self.client.clone() {
            client.create_index(index, None).await.unwrap();
        }
    }

    //Update data if it exists and add if not
    pub async fn add_data(
        &self,
        index: &str,
        data_for_put: BlazzyData,
        primary_key: Option<&str>
    ) {
        if let Some(client) = self.client.clone() {
            let index = client.index(index);
            let data = index.search().execute::<DataFile>().await.expect("Failed to execute search");
            let mut id = 0;
            if !data.hits.is_empty() {
                id = data.hits.iter().last().unwrap().result.id + 1
            }
            if let Some(hit) = data.hits.iter().find(|&x| x.result.data.get_path() == data_for_put.get_path()) {
                index.add_documents(&[
                    DataFile {
                        id: hit.result.id,
                        data: data_for_put
                    }
                ], primary_key).await.unwrap();

            } else {
                index.add_documents(&[
                    DataFile {
                        id,
                        data: data_for_put
                    }
                ], primary_key).await.unwrap();
            }

        }
    }

    pub async fn get_data(&self, index: &str) -> Option<SearchResults<DataFile>> {
        if let Some(client) = self.client.clone() {
            let index = client.index(index);
            return Some(index.search().execute::<DataFile>().await.expect("Failed to execute search"))
        }
        None
    }
}

#[derive(Clone)]
pub struct MeilisearchMasterKey(String);

impl MeilisearchMasterKey {
    pub fn gen() -> MeilisearchMasterKey {
        let pg = PasswordGenerator {
            length: 16,
            numbers: true,
            lowercase_letters: true,
            uppercase_letters: true,
            symbols: true,
            spaces: false,
            exclude_similar_characters: false,
            strict: true,
        };

        MeilisearchMasterKey(pg.generate_one().unwrap())
    }
}

impl Display for MeilisearchMasterKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone)]
pub struct MeilisearchHost(String, u16);

impl Default for MeilisearchHost {
    fn default() -> Self {
        MeilisearchHost("localhost".to_string(), 7700)
    }
}

impl MeilisearchHost {
    pub fn new(ip: &str, port: u16) -> Self {
        MeilisearchHost(ip.to_string(), port)
    }
}

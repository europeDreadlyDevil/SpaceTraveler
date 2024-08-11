use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use atomic_refcell::AtomicRefCell;
use futures::{FutureExt, SinkExt, StreamExt};
use serde_json::json;
use tauri::{command, GlobalWindowEvent, Menu, WindowEvent};
use tokio::fs::{OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::channel;
use tokio::time::sleep;
use tracing::{error, info};
use websocket::{ClientBuilder, Message, OwnedMessage, WebSocketResult};
use starship_plugin_api::api::StarShipPluginAPI;
use crate::blazzy_client::{BlazzyClient, BlazzyData};
use crate::config_manager::{AppState, ConfigManager};
use crate::meilisearch_client::MeilisearchClient;
use crate::meilisearch_runner::runner::{MeilisearchHost, MeilisearchMasterKey, MeilisearchRunner};
use crate::tasker::{Tasker, TaskerError,};
use crate::ws_connector::WsConnector;

pub struct App {
    config: ConfigManager,
    ws_connector: WsConnector,
    plugins: Vec<Box<dyn StarShipPluginAPI>>,

}

impl App {
    pub async fn init_conf(conf_path: Option<PathBuf>) -> Self {
        let conf_path = conf_path.unwrap_or(PathBuf::from(std::env::current_exe().unwrap().parent().unwrap().join(".conf.toml")));
        {
            OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(conf_path.clone())
                .await.unwrap();
        }
        let mut config = ConfigManager::new(conf_path).await;
        config.setup().await;
        Self{
            config,
            plugins: vec![],
            ws_connector: WsConnector::init(),
        }
    }

    pub async fn get_state(&mut self) -> AppState {
        self.config.get_state().await
    }

    pub async fn conf_first_setup(&mut self) {
        self.config.setup().await;
    }

    pub async fn default_run(&mut self) {

        let tasker_app = tokio::task::spawn(async {
            let mut tasker = Tasker::init();
            if let Err(e) = tasker.safe_run().await {
                error!(name: "Tasker run error", "Error: {}", e);
            }
        });

        let mut blazzy_client = BlazzyClient::init();
        let blazzy_exe_path = blazzy_client.get_exe_path();

        let mut meilisearch_client = MeilisearchClient::init();
        let meili_exe_path = meilisearch_client.get_exe_path();
        let master_key = format!("--master-key={}",meilisearch_client.get_master_key());
        let db_path = format!("--db-path={}",meilisearch_client.get_db_path().display());
        let dump = format!("--dump-dir={}",meilisearch_client.get_dump_dir().display());

        self.ws_connector.connect("tasker","ws://127.0.0.1:5000/", None).await;

        let mut tasker_run_blazzy = "run_app".to_string();
        tasker_run_blazzy.push_str(
            &json!({
                "app": blazzy_exe_path,
                "args": ["-p", "C:/", "-c", "w"],
                "window": false
            }).to_string()
        );

        let mut tasker_run_meili = "run_app".to_string();
        tasker_run_meili.push_str(
            &json!({
                "app": meili_exe_path,
                "args": [master_key, db_path, dump],
                "window": false
            }).to_string()
        );

        self.ws_connector.send("tasker", &tasker_run_blazzy).await;
        self.ws_connector.send("tasker", &tasker_run_meili).await;

        let (tx, mut rx) = channel(1024);

        self.ws_connector.connect("blazzy", "ws://127.0.0.1:8080", Some(tx.clone())).await;
        if let Some(con) = self.ws_connector.get_connection("blazzy") {
            blazzy_client.connect_channel((con.clone(), rx))
        }

        self.ws_connector.send("tasker", "observe { \"main_app\": \"SpaceTraveler\", \"sub_apps\": [\"blazzy\", \"search_engine\"], \"safe\": \"WoSafe\" }").await;
        //blazzy_client.listen();

        tokio::time::sleep(Duration::from_secs(2)).await;

        tokio::select! {
            b = meilisearch_client.run() => {
                if b {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    tokio::select! {
                        _ = meilisearch_client.create_index("files") => {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            tokio::select! {
                                _ = meilisearch_client.add_data("files", BlazzyData::new("file.tx".to_string(), "fiee.txt".to_string(), None), Some("id")) => {
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                    let data = meilisearch_client.get_data("files").await;
                                    info!("{:?}", data);
                                }
                            }
                        }
                    }
                }
            }
        }

        let menu = Menu::new();

        tauri::Builder::default()
            .menu(menu)
            .invoke_handler(tauri::generate_handler![call])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");


    }

    pub async fn call() {
        info!("Calling")
    }

}

#[command]
pub async fn call() {
    App::call().await;
}
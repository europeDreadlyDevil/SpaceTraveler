use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use atomic_refcell::AtomicRefCell;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Sender, Receiver};
use tracing::{error, info};
use websocket::OwnedMessage;

#[derive(Serialize, Deserialize, Debug)]
pub struct BlazzyData {
    file_path: String,
    file_name: String,
    metadata: Option<Metadata>
}

impl BlazzyData {
    pub fn new(
        file_path: String,
        file_name: String,
        metadata: Option<Metadata>
    ) -> BlazzyData {
        Self {
            file_path,
            file_name,
            metadata
        }
    }
    pub fn get_path(&self) -> PathBuf {
        PathBuf::from(self.file_path.clone())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    file_type: String,
    is_dir: bool,
    is_file: bool,
    is_symlink: bool,
    size: u64,
    permissions: String,
    modified: String,
    accessed: String,
    created: String
}
pub struct BlazzyClient {
    sender: Option<Sender<OwnedMessage>>,
    recv: Option<Arc<AtomicRefCell<Receiver<OwnedMessage>>>>,
    exe_path: PathBuf
}

impl BlazzyClient {
    pub fn init() -> Self {
        let exe_path = std::env::current_exe().unwrap()
            .parent().unwrap()
            .join("services/blazzy").join("blazzy.exe");
        {
            if !exe_path.exists() {
                let blazzy_path = exe_path.parent().unwrap();
                if !blazzy_path.exists() {
                    let services_path = blazzy_path.parent().unwrap();
                    if !services_path.exists() {
                        std::fs::create_dir(services_path).unwrap();
                    }
                    std::fs::create_dir(blazzy_path).unwrap();
                }
                let mut file = File::create(&exe_path).unwrap();
                file.write_all(include_bytes!("../../assets/blazzy.exe")).unwrap();
            }
        }

        Self {
            sender: None,
            recv: None,
            exe_path
        }

    }

    pub fn get_exe_path(&self) -> &PathBuf {
        &self.exe_path
    }

    pub fn connect_channel(&mut self, channel: (Sender<OwnedMessage>, Receiver<OwnedMessage>)) {
        self.sender = Some(channel.0);
        self.recv = Some(Arc::new(AtomicRefCell::new(channel.1)));
    }

    pub async fn send(&self, message: &str) {
        if let Some(sender) = self.sender.clone() {
            if let Err(e) = sender.send(OwnedMessage::Text(message.to_string())).await {
                error!("{}", e)
            }
        }
    }

    pub fn listen(&mut self) {
        if let Some(rx) = self.recv.clone() {
            tokio::task::spawn(async move {
                //let mut message_stack = vec![];

                while let Some(message) = rx.borrow_mut().recv().await {
                    match message {
                        OwnedMessage::Text(data) => {
                            info!("{data}")
                        }
                        _ => {}
                    }
                }
            });
        }
    }
}
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tokio::{sync::oneshot, task::JoinHandle};

use crate::{database, settings};

#[derive(Debug, Clone)]
pub struct GatewayStatus {
    pub running: bool,
    pub status: String,
    pub error: Option<String>,
}

impl Default for GatewayStatus {
    fn default() -> Self {
        Self { running: false, status: "idle".into(), error: None }
    }
}

pub struct GatewayHandle {
    pub shutdown: Option<oneshot::Sender<()>>,
    pub _task: JoinHandle<()>,
}

pub struct Runtime {
    pub gateway_status: Mutex<GatewayStatus>,
    pub gateway_handle: Mutex<Option<GatewayHandle>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            gateway_status: Mutex::new(GatewayStatus::default()),
            gateway_handle: Mutex::new(None),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub settings_path: PathBuf,
    pub logs_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub runtime: Arc<Runtime>,
}

impl AppState {
    pub fn init() -> Result<Self, String> {
        let dir = dirs::data_local_dir()
            .ok_or("Cannot find local data directory")?
            .join("Gateway Switch");

        match Self::init_at(dir.clone()) {
            Ok(s) => Ok(s),
            Err(_) => Self::init_at(std::env::temp_dir().join("Gateway Switch")),
        }
    }

    fn init_at(data_dir: PathBuf) -> Result<Self, String> {
        let db_path = data_dir.join("gateway.db");
        let settings_path = data_dir.join("settings.json");
        let logs_dir = data_dir.join("logs");
        let backups_dir = data_dir.join("backups");

        for dir in [&data_dir, &logs_dir, &backups_dir] {
            fs::create_dir_all(dir).map_err(|e| format!("mkdir {:?}: {e}", dir))?;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("chmod {:?}: {e}", dir))?;
        }

        if let Err(e) = database::initialize(&db_path) {
            let _ = fs::remove_file(&db_path);
            database::initialize(&db_path)
                .map_err(|e2| format!("db init failed: {e}; retry: {e2}"))?;
        }
        settings::load(&settings_path)?;

        Ok(Self {
            data_dir,
            db_path,
            settings_path,
            logs_dir,
            backups_dir,
            runtime: Arc::new(Runtime::new()),
        })
    }
}

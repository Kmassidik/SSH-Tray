use std::env;
use std::path::PathBuf;

mod editor;
mod parser;

pub use editor::{delete_host, save_host};
pub use parser::load_hosts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub alias: String,
    pub group: Option<String>,
    pub aliases: Vec<String>,
    pub host_name: String,
    pub user: String,
    pub port: String,
    pub identity_file: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDraft {
    pub aliases: String,
    pub host_name: String,
    pub user: String,
    pub port: String,
    pub identity_file: String,
    pub group: String,
}

fn config_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .ok_or_else(|| "HOME is not set; cannot locate ~/.ssh/config".to_owned())?;
    Ok(PathBuf::from(home).join(".ssh").join("config"))
}

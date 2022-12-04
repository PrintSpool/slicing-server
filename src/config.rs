use directories::ProjectDirs;
use eyre::eyre;
use eyre::Result;
use jwt_simple::prelude::ES256KeyPair;
use self_host_space::KeyManager;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

#[derive(Serialize, Deserialize)]
pub struct ClientKey {
    pub id: String,
    pub label: String,
    pub public_key_pem: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    /// A mapping of JWT key ids to public key PEMs which are authorized to access the slicing server
    pub authorized_keys: HashMap<String, ClientKey>,
}

impl Config {
    fn config_path() -> Result<PathBuf> {
        let dirs = directories()?;
        let config_path = dirs.config_dir().join("config.toml");
        Ok(config_path)
    }

    pub async fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        let config = if config_path.exists() {
            toml::from_str(&fs::read_to_string(config_path).await?)?
        } else {
            Config::default()
        };

        Ok(config)
    }

    pub async fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        fs::write(config_path, toml::to_string_pretty(self)?).await?;

        Ok(())
    }

    pub fn add_key(
        &mut self,
        server_keys: &KeyManager,
        label: String,
    ) -> Result<(String, &ClientKey)> {
        let id = nanoid::nanoid!();

        let key_pair = ES256KeyPair::generate();

        let signing_key_pem = key_pair
            .to_pem()
            .map_err(|_| eyre!("Failed to generate PEM for client signing key"))?;

        let public_key_pem = key_pair
            .public_key()
            .to_pem()
            .map_err(|_| eyre!("Failed to generate PEM for client public key"))?;

        if self.authorized_keys.contains_key(&id) {
            return Err(eyre!("authorized_keys hash collision"))?;
        }

        let client_key = self.authorized_keys.entry(id.clone()).or_insert(ClientKey {
            id,
            label,
            public_key_pem,
        });

        let invite_json = serde_json::json!({
            "id": client_key.id,
            "sk": signing_key_pem,
            "server_pk": server_keys.server_identity.identity_public_key,
        });

        let invite_token = bs58::encode(serde_json::to_string(&invite_json)?).into_string();

        Ok((invite_token, client_key))
    }
}

pub fn directories() -> Result<ProjectDirs> {
    let dirs = ProjectDirs::from("", "", "slicer-server")
        .ok_or_else(|| eyre!("Unable to get application directories, is this OS supported?"))?;

    Ok(dirs)
}

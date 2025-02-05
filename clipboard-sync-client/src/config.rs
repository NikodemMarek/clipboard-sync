use clap::Parser;
use rsa::{
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePublicKey},
    RsaPrivateKey, RsaPublicKey,
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub relay: String,

    pub client_key: RsaPrivateKey,
    pub client_pub_key: RsaPublicKey,
    pub client_id: Box<str>,
    pub peers_keys: std::collections::HashMap<Box<str>, RsaPublicKey>,
}

pub fn get() -> Config {
    let cli_args = CliArgs::parse();
    let config_file = cli_args
        .config
        .clone()
        .map(|p| p.into_os_string().to_string_lossy().to_string())
        .unwrap_or_else(|| "~/.config/clipboard-sync/config.toml".to_string());

    let config: FileConfig = config::Config::builder()
        .add_source(DefaultConfig)
        .add_source(config::File::with_name(&shellexpand::tilde(&config_file)))
        .add_source(cli_args)
        .build()
        .expect("failed to build config")
        .try_deserialize()
        .expect("failed to deserialize config");

    finalize_config(config)
}

fn compute_id(key: &RsaPublicKey) -> Box<str> {
    key.to_public_key_der()
        .map(|d| sha256::digest(d.as_bytes()).into())
        .expect("failed to compute id")
}

fn get_id_key_from_file(path: impl Into<PathBuf>) -> (Box<str>, RsaPublicKey) {
    let key =
        RsaPublicKey::read_public_key_pem_file(path.into()).expect("failed to get a key from file");
    (compute_id(&key), key)
}

fn finalize_config(raw_config: FileConfig) -> Config {
    let relay = raw_config.relay.expect("relay address not provided");

    let client_key = rsa::RsaPrivateKey::read_pkcs8_pem_file(
        raw_config.client_key.expect("client key path not provided"),
    )
    .expect("failed to get a client key from file");
    let client_pub_key = rsa::RsaPublicKey::from(&client_key);
    let client_id = compute_id(&client_pub_key);

    let peers_keys = raw_config
        .peers_keys
        .map_or_else(std::collections::HashMap::new, |keys| {
            keys.iter()
                .map(get_id_key_from_file)
                .collect::<std::collections::HashMap<Box<str>, rsa::RsaPublicKey>>()
        });

    Config {
        relay,
        client_key,
        client_pub_key,
        client_id,
        peers_keys,
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct FileConfig {
    pub relay: Option<String>,

    pub client_key: Option<PathBuf>,
    pub peers_keys: Option<Vec<PathBuf>>,
}
impl config::Source for FileConfig {
    fn clone_into_box(&self) -> Box<dyn config::Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<config::Map<String, config::Value>, config::ConfigError> {
        use config::Value;
        let mut map = std::collections::HashMap::new();
        if let Some(relay) = &self.relay {
            map.insert(
                "relay".into(),
                Value::new(Some(&"relay".into()), relay.clone()),
            );
        }
        if let Some(peers_keys) = &self.peers_keys {
            let peers_keys = peers_keys
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<String>>();

            if !peers_keys.is_empty() {
                map.insert(
                    "peers_keys".into(),
                    Value::new(Some(&"peers_keys".into()), peers_keys),
                );
            }
        }
        if let Some(client_key) = &self.client_key {
            map.insert(
                "client_key".into(),
                Value::new(
                    Some(&"client_key".into()),
                    client_key.to_string_lossy().to_string(),
                ),
            );
        }
        Ok(map)
    }
}

#[derive(Debug, Clone)]
struct DefaultConfig;
impl config::Source for DefaultConfig {
    fn clone_into_box(&self) -> Box<dyn config::Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<config::Map<String, config::Value>, config::ConfigError> {
        use config::Value;
        Ok(std::collections::HashMap::from([(
            "relay".into(),
            Value::new(Some(&"relay".into()), "130.61.88.218:5200"),
        )]))
    }
}

#[derive(clap::Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct CliArgs {
    #[arg(short, long)]
    relay: Option<String>,

    #[arg(short, long)]
    client_key: Option<PathBuf>,

    #[arg(short, long)]
    peers_keys: Vec<PathBuf>,

    #[arg(long)]
    config: Option<PathBuf>,
}
impl config::Source for CliArgs {
    fn clone_into_box(&self) -> Box<dyn config::Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<config::Map<String, config::Value>, config::ConfigError> {
        use config::Value;

        let mut map = std::collections::HashMap::new();
        if let Some(relay) = &self.relay {
            map.insert(
                "relay".into(),
                Value::new(Some(&"relay".into()), relay.clone()),
            );
        }

        let peers_keys = self
            .peers_keys
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<String>>();
        if !peers_keys.is_empty() {
            map.insert(
                "peers_keys".into(),
                Value::new(Some(&"peers_keys".into()), peers_keys),
            );
        }

        if let Some(decryption_key_path) = &self.client_key {
            map.insert(
                "client_key".into(),
                Value::new(
                    Some(&"client_key".into()),
                    decryption_key_path.to_string_lossy().to_string(),
                ),
            );
        }
        Ok(map)
    }
}

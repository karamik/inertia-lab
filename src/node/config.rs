use std::path::PathBuf;

#[derive(Clone)]
pub struct NodeConfig {
    pub datadir: PathBuf,
    pub node_name: String,
    pub mode: String,
    pub enable_bluetooth: bool,
    pub enable_wifi_stego: bool,
    pub enable_ultrasound: bool,
    pub enable_dns_spore: bool,
    pub enable_lora: bool,
    pub enable_astro: bool,
    pub bridge_port: u16,
    pub bootstrap_peers: Vec<String>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            datadir: PathBuf::from("./.inertia"),
            node_name: "inertia-node".to_string(),
            mode: "seed".to_string(),
            enable_bluetooth: true,
            enable_wifi_stego: true,
            enable_ultrasound: false,
            enable_dns_spore: false,
            enable_lora: false,
            enable_astro: false,
            bridge_port: 18888,
            bootstrap_peers: Vec::new(),
        }
    }
}

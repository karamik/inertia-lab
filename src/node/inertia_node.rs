use crate::KeyPair;
use crate::node::config::NodeConfig;
use log::info;

pub struct InertiaNode {
    keypair: KeyPair,
    config: NodeConfig,
    running: bool,
}

impl InertiaNode {
    pub fn new(keypair: KeyPair, config: NodeConfig) -> Self {
        Self {
            keypair,
            config,
            running: false,
        }
    }
    
    pub fn init(&mut self) -> crate::Result<()> {
        info!("Initializing Inertia node...");
        // TODO: Implement initialization
        Ok(())
    }
    
    pub async fn start(&mut self) -> crate::Result<()> {
        info!("Starting Inertia node...");
        self.running = true;
        // TODO: Implement start logic
        Ok(())
    }
    
    pub async fn stop(&mut self) -> crate::Result<()> {
        info!("Stopping Inertia node...");
        self.running = false;
        Ok(())
    }
}

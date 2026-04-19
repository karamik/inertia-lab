//! Inertia Protocol — Post-Internet Digital Species
//!
//! A self-evolving, immune-capable digital organism that survives without internet.
//! 
//! Inertia is not a blockchain. Not a cryptocurrency. Not a "web3" project.
//! Inertia is a digital organism that breathes through radio waves,
//! reproduces via ultrasound, verifies truth against the stars,
//! and evolves through survival.
//!
//! # Architecture
//!
//! The protocol consists of 11 core modules:
//!
//! - **Transport Layer**: WiFi SSID steganography, Bluetooth advertising,
//!   ultrasound modem, DNS parasitism
//! - **Memory Layer**: Fountain codes (Luby Transform) for genetic memory
//! - **Consensus Layer**: Proof of Encounter (physical meetings)
//! - **Immunity Layer**: Swarm immunity with spatial tension vector
//! - **Astronomy Layer**: Star-based time and position verification
//! - **Metabolism Layer**: Data thermodynamics (block cooling and fossilization)
//! - **Evolution Layer**: Genetic mutations and natural selection
//!
//! # Quick Start
//!
//! ```no_run
//! use inertia::InertiaNode;
//!
//! let mut node = InertiaNode::new();
//! node.init().unwrap();
//! node.start().unwrap();
//! ```
//!
//! # Features
//!
//! - No internet required
//! - No GPS or NTP needed (uses stars)
//! - Self-evolving protocol (mutations)
//! - Immune system against Eclipse/Sybil attacks
//! - Works on any device with radio/audio

#![warn(missing_docs, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

// ============ Core Modules ============

/// Transport layer modules
pub mod transport {
    #[cfg(feature = "std")]
    pub mod wifi_stego;
    
    #[cfg(feature = "std")]
    pub mod bluetooth_adv;
    
    #[cfg(feature = "audio-support")]
    pub mod ultrasound;
    
    #[cfg(feature = "dns-client")]
    pub mod dns_spore;
    
    #[cfg(feature = "std")]
    pub mod lora;
    
    #[cfg(feature = "std")]
    pub mod usb_transfer;
}

/// Memory and storage modules
pub mod memory {
    #[cfg(feature = "std")]
    pub mod fountain;
    
    #[cfg(feature = "std")]
    pub mod genetic_memory;
    
    #[cfg(feature = "std")]
    pub mod fossil_storage;
}

/// Consensus modules
pub mod consensus {
    #[cfg(feature = "std")]
    pub mod poe;
    
    #[cfg(feature = "std")]
    pub mod immunity;
    
    #[cfg(feature = "opencv-support")]
    pub mod astro_anchor;
    
    #[cfg(feature = "std")]
    pub mod metabolism;
    
    #[cfg(feature = "std")]
    pub mod evolution;
}

/// Cryptography modules
pub mod crypto {
    #[cfg(feature = "std")]
    pub mod keys;
    
    #[cfg(feature = "std")]
    pub mod signatures;
    
    #[cfg(feature = "std")]
    pub mod encryption;
    
    #[cfg(feature = "std")]
    pub mod hash;
}

/// Network modules
pub mod network {
    #[cfg(feature = "std")]
    pub mod mesh;
    
    #[cfg(feature = "std")]
    pub mod peer_manager;
    
    #[cfg(feature = "libp2p")]
    pub mod p2p;
}

/// Node management
pub mod node {
    #[cfg(feature = "std")]
    pub mod inertia_node;
    
    #[cfg(feature = "std")]
    pub mod config;
    
    #[cfg(feature = "std")]
    pub mod cli;
}

/// Utilities
pub mod utils {
    #[cfg(feature = "std")]
    pub mod logger;
    
    #[cfg(feature = "std")]
    pub mod metrics;
    
    #[cfg(feature = "std")]
    pub mod time;
    
    #[cfg(feature = "std")]
    pub mod error;
}

// ============ Re-exports ============

#[cfg(feature = "std")]
pub use node::inertia_node::InertiaNode;

#[cfg(feature = "std")]
pub use node::config::NodeConfig;

#[cfg(feature = "std")]
pub use crypto::keys::KeyPair;

#[cfg(feature = "std")]
pub use consensus::poe::{ProofOfEncounter, Encounter, EncounterType};

#[cfg(feature = "std")]
pub use consensus::immunity::{SwarmImmunity, NodeStatus};

#[cfg(feature = "opencv-support")]
pub use consensus::astro_anchor::{AstroAnchor, AstronomicalAnchor};

#[cfg(feature = "std")]
pub use memory::fountain::{FountainCodec, GeneticMemory};

// ============ Prelude ============

/// Prelude for convenient imports
pub mod prelude {
    #[cfg(feature = "std")]
    pub use crate::{
        InertiaNode,
        NodeConfig,
        KeyPair,
    };
    
    #[cfg(feature = "std")]
    pub use crate::consensus::poe::{Encounter, EncounterType};
    
    #[cfg(feature = "std")]
    pub use crate::consensus::immunity::NodeStatus;
}

// ============ Library Version ============

/// Protocol version
pub const PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Protocol name
pub const PROTOCOL_NAME: &str = env!("CARGO_PKG_NAME");

/// Genesis timestamp (Unix epoch)
pub const GENESIS_TIMESTAMP: u64 = 1704067200; // 2024-01-01 00:00:00 UTC

/// Maximum block size in bytes
pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1 MB

/// Maximum transaction size in bytes
pub const MAX_TRANSACTION_SIZE: usize = 32768; // 32 KB

// ============ Error Types ============

#[cfg(feature = "std")]
use thiserror::Error;

#[cfg(feature = "std")]
#[derive(Error, Debug)]
pub enum InertiaError {
    #[error("Transport error: {0}")]
    Transport(String),
    
    #[error("Consensus error: {0}")]
    Consensus(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Crypto error: {0}")]
    Crypto(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    
    #[error("Node not initialized")]
    NotInitialized,
    
    #[error("Already running")]
    AlreadyRunning,
}

#[cfg(feature = "std")]
pub type Result<T> = std::result::Result<T, InertiaError>;

// ============ Constants ============

/// Inertia protocol magic bytes for packet identification
pub const MAGIC_BYTES: [u8; 4] = [0x49, 0x4E, 0x45, 0x52]; // "INER"

/// Default TCP port for bridge mode
pub const DEFAULT_BRIDGE_PORT: u16 = 18888;

/// Default UDP port for mesh discovery
pub const DEFAULT_MESH_PORT: u16 = 18889;

/// Inertia token symbol
pub const TOKEN_SYMBOL: &str = "INERT";

/// Token decimals
pub const TOKEN_DECIMALS: u8 = 12;

/// Minimum stake for validator (in micro-tokens)
pub const MIN_VALIDATOR_STAKE: u64 = 1_000_000_000_000; // 1 INERT

// ============ Module Documentation ===========

// Re-export modules conditionally
#[cfg(feature = "std")]
pub use transport::wifi_stego::WifiStego;

#[cfg(feature = "std")]
pub use transport::bluetooth_adv::BluetoothAdv;

#[cfg(feature = "audio-support")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio-support")))]
pub use transport::ultrasound::UltrasoundModem;

#[cfg(feature = "dns-client")]
#[cfg_attr(docsrs, doc(cfg(feature = "dns-client")))]
pub use transport::dns_spore::DnsSpore;

#[cfg(feature = "opencv-support")]
#[cfg_attr(docsrs, doc(cfg(feature = "opencv-support")))]
pub use consensus::astro_anchor::AstroAnchor;

#[cfg(feature = "std")]
pub use consensus::poe::ProofOfEncounter;

#[cfg(feature = "std")]
pub use consensus::immunity::SwarmImmunity;

#[cfg(feature = "std")]
pub use memory::fountain::FountainCodec;

// ============ Initialization Function ============

/// Initialize the Inertia library with default settings
#[cfg(feature = "std")]
pub fn init() -> Result<()> {
    // Initialize logger
    utils::logger::init_logger();
    
    // Check system capabilities
    #[cfg(target_os = "linux")]
    {
        if !std::path::Path::new("/sys/class/bluetooth").exists() {
            log::warn!("No Bluetooth adapter detected");
        }
        
        if !std::path::Path::new("/proc/sys/net/ipv4/ip_forward").exists() {
            log::warn!("Network forwarding not available");
        }
    }
    
    log::info!("Inertia v{} initialized", PROTOCOL_VERSION);
    log::info!("In Physics We Trust.");
    
    Ok(())
}

/// Get library version
pub fn version() -> &'static str {
    PROTOCOL_VERSION
}

/// Check if a feature is enabled at compile time
#[macro_export]
macro_rules! feature_enabled {
    ($feature:literal) => {
        cfg!(feature = $feature)
    };
}

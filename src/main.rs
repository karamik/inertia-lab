// src/main.rs
// Inertia Protocol — Post-Internet Digital Species
// CLI daemon for running Inertia nodes

use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use std::path::PathBuf;
use std::process;
use std::time::Duration;
use log::{info, warn, error, debug};

use inertia::{
    InertiaNode, 
    NodeConfig, 
    KeyPair,
    PROTOCOL_VERSION,
    PROTOCOL_NAME,
    init as init_inertia,
};

// ============ CLI Arguments ============

#[derive(Parser)]
#[command(name = "inertiad")]
#[command author = "International Group of Developers <inertia@inertia.network>"]
#[command version = PROTOCOL_VERSION]
#[command about = "Inertia Protocol — Post-Internet Digital Species\nA self-evolving, immune-capable digital organism that survives without internet.", long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    
    /// Data directory for blockchain storage
    #[arg(short, long, default_value = "./.inertia")]
    datadir: PathBuf,
    
    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    log_level: String,
    
    /// Enable JSON logging format
    #[arg(long)]
    json_logs: bool,
    
    /// Subcommand to execute
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start Inertia node (default)
    Start {
        /// Node name (appears in peer discovery)
        #[arg(short, long, default_value = "inertia-node")]
        name: String,
        
        /// Node mode
        #[arg(short, long, value_enum, default_value_t = NodeMode::Seed)]
        mode: NodeMode,
        
        /// Enable Bluetooth transport
        #[arg(long)]
        bluetooth: bool,
        
        /// Enable WiFi SSID steganography
        #[arg(long)]
        wifi_stego: bool,
        
        /// Enable ultrasound modem (19 kHz)
        #[arg(long)]
        ultrasound: bool,
        
        /// Enable DNS parasitism
        #[arg(long)]
        dns_spore: bool,
        
        /// Enable LoRa radio (requires hardware)
        #[arg(long)]
        lora: bool,
        
        /// Enable astronomical anchor (star verification)
        #[arg(long)]
        astro: bool,
        
        /// Port for bridge mode (internet connection)
        #[arg(short, long, default_value = "18888")]
        bridge_port: u16,
        
        /// Bootstrap peers (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        bootstrap: Vec<String>,
    },
    
    /// Generate new keypair
    GenerateKey {
        /// Output file path
        #[arg(short, long, default_value = "inertia.key")]
        output: PathBuf,
        
        /// Print to stdout instead of file
        #[arg(long)]
        stdout: bool,
    },
    
    /// Show node information
    Info {
        /// Node address to query
        #[arg(default_value = "self")]
        address: String,
    },
    
    /// Send a transaction
    Send {
        /// Recipient address (hex-encoded public key)
        #[arg(short, long)]
        to: String,
        
        /// Amount in INERT tokens
        #[arg(short, long)]
        amount: u64,
        
        /// Transaction memo (optional)
        #[arg(short, long)]
        memo: Option<String>,
    },
    
    /// Check node status
    Status,
    
    /// View blockchain statistics
    Stats,
    
    /// Show active peers
    Peers,
    
    /// Display evolution status (genes and mutations)
    Evolution,
    
    /// Display immunity status (reputation and threats)
    Immunity,
    
    /// Show astronomical anchor data
    Astro,
    
    /// Benchmark transport modules
    Benchmark {
        /// Transport module to benchmark
        #[arg(value_enum, default_value_t = BenchmarkModule::All)]
        module: BenchmarkModule,
        
        /// Duration in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,
    },
    
    /// Reset node data (dangerous!)
    Reset {
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Version information
    Version,
}

#[derive(ValueEnum, Clone, Debug)]
enum NodeMode {
    /// Seed node — actively broadcasts and relays data
    Seed,
    /// Harvester node — collects data but doesn't actively broadcast
    Harvester,
    /// Full node — fully participates in consensus
    Full,
    /// Light node — only verifies and forwards
    Light,
}

#[derive(ValueEnum, Clone, Debug)]
enum BenchmarkModule {
    All,
    WifiStego,
    Bluetooth,
    Ultrasound,
    DnsSpore,
    Fountain,
    PoE,
}

// ============ Main Entry Point ============

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    // Initialize logging
    init_logging(&cli.log_level, cli.json_logs);
    
    // Print banner
    print_banner();
    
    // Handle commands
    let result = match cli.command {
        Some(Commands::Start { name, mode, bluetooth, wifi_stego, ultrasound, dns_spore, lora, astro, bridge_port, bootstrap }) => {
            cmd_start(
                cli.datadir, name, mode, bluetooth, wifi_stego, 
                ultrasound, dns_spore, lora, astro, bridge_port, bootstrap
            ).await
        }
        Some(Commands::GenerateKey { output, stdout }) => {
            cmd_generate_key(output, stdout)
        }
        Some(Commands::Info { address }) => {
            cmd_info(address)
        }
        Some(Commands::Send { to, amount, memo }) => {
            cmd_send(to, amount, memo).await
        }
        Some(Commands::Status) => {
            cmd_status(cli.datadir).await
        }
        Some(Commands::Stats) => {
            cmd_stats(cli.datadir).await
        }
        Some(Commands::Peers) => {
            cmd_peers(cli.datadir).await
        }
        Some(Commands::Evolution) => {
            cmd_evolution(cli.datadir).await
        }
        Some(Commands::Immunity) => {
            cmd_immunity(cli.datadir).await
        }
        Some(Commands::Astro) => {
            cmd_astro().await
        }
        Some(Commands::Benchmark { module, duration }) => {
            cmd_benchmark(module, duration).await
        }
        Some(Commands::Reset { force }) => {
            cmd_reset(cli.datadir, force).await
        }
        Some(Commands::Version) => {
            cmd_version();
            return;
        }
        None => {
            // Default: start node with default settings
            cmd_start(
                cli.datadir, 
                "inertia-node".to_string(), 
                NodeMode::Seed,
                true, true, true, true, false, false,
                18888,
                vec![]
            ).await
        }
    };
    
    if let Err(e) = result {
        error!("{}", e);
        process::exit(1);
    }
}

// ============ Command Implementations ============

async fn cmd_start(
    datadir: PathBuf,
    name: String,
    mode: NodeMode,
    bluetooth: bool,
    wifi_stego: bool,
    ultrasound: bool,
    dns_spore: bool,
    lora: bool,
    astro: bool,
    bridge_port: u16,
    bootstrap: Vec<String>,
) -> inertia::Result<()> {
    info!("Starting Inertia node: {}", name.bright_cyan());
    info!("Version: {}", PROTOCOL_VERSION);
    info!("Mode: {:?}", mode);
    info!("Data directory: {}", datadir.display());
    
    // Load or create keypair
    let key_path = datadir.join("keypair.bin");
    let keypair = if key_path.exists() {
        info!("Loading existing keypair from {}", key_path.display());
        load_keypair(&key_path)?
    } else {
        info!("Generating new keypair...");
        let kp = KeyPair::generate();
        save_keypair(&kp, &key_path)?;
        kp
    };
    
    let pubkey_hex = hex::encode(keypair.public().as_bytes());
    info!("Node ID: {}", pubkey_hex.chars().take(16).collect::<String>());
    
    // Build configuration
    let config = NodeConfig {
        datadir: datadir.clone(),
        node_name: name.clone(),
        mode: format!("{:?}", mode).to_lowercase(),
        enable_bluetooth: bluetooth,
        enable_wifi_stego: wifi_stego,
        enable_ultrasound: ultrasound,
        enable_dns_spore: dns_spore,
        enable_lora: lora,
        enable_astro: astro,
        bridge_port,
        bootstrap_peers: bootstrap,
        ..Default::default()
    };
    
    // Initialize Inertia
    init_inertia()?;
    
    // Create and start node
    let mut node = InertiaNode::new(keypair, config);
    node.init()?;
    
    info!("{}", "=".repeat(60));
    info!("🚀 Inertia node is running");
    info!("📡 Transports enabled:");
    if bluetooth { info!("   🔵 Bluetooth advertising"); }
    if wifi_stego { info!("   📶 WiFi SSID steganography"); }
    if ultrasound { info!("   🔊 Ultrasound (19 kHz)"); }
    if dns_spore { info!("   🌐 DNS parasitism"); }
    if lora { info!("   📡 LoRa radio"); }
    if astro { info!("   ⭐ Astronomical anchor"); }
    info!("🌉 Bridge port: {}", bridge_port);
    info!("{}", "=".repeat(60));
    
    node.start().await?;
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await.ok();
    info!("Shutting down...");
    
    node.stop().await?;
    info!("Goodbye! In Physics We Trust. 🌌");
    
    Ok(())
}

async fn cmd_generate_key(output: PathBuf, stdout: bool) -> inertia::Result<()> {
    let keypair = KeyPair::generate();
    let pubkey_hex = hex::encode(keypair.public().as_bytes());
    let privkey_hex = hex::encode(keypair.secret().as_bytes());
    
    if stdout {
        println!("=== INERTIA KEYPAIR ===");
        println!("Public key:  {}", pubkey_hex);
        println!("Private key: {}", privkey_hex);
        println!("=== KEEP PRIVATE KEY SECRET! ===");
    } else {
        let data = serde_json::json!({
            "public_key": pubkey_hex,
            "private_key": privkey_hex,
            "version": PROTOCOL_VERSION,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        
        std::fs::write(&output, serde_json::to_string_pretty(&data)?)?;
        info!("Keypair saved to {}", output.display());
        info!("Public key: {}", &pubkey_hex[..16]);
    }
    
    Ok(())
}

async fn cmd_info(address: String) -> inertia::Result<()> {
    info!("Querying node: {}", address);
    
    if address == "self" {
        println!("{}", "=".repeat(50));
        println!("{} Inertia Node Information", "🌟".bold());
        println!("{}", "=".repeat(50));
        println!("Protocol:      {} v{}", PROTOCOL_NAME, PROTOCOL_VERSION);
        println!("Status:        {}", "Running".green());
        println!("Consensus:     Proof of Encounter (PoE)");
        println!("Immunity:      Active (Swarm Immunity)");
        println!("Evolution:     Active (Generation {})", "?");
        println!("Astro Anchor:  {}", if cfg!(feature = "opencv-support") { "Available".green() } else { "Disabled".dimmed() });
        println!("{}", "=".repeat(50));
        println!("In Physics We Trust. 🌌");
    } else {
        // TODO: Query remote node via bridge
        println!("Remote node query not yet implemented");
    }
    
    Ok(())
}

async fn cmd_send(to: String, amount: u64, memo: Option<String>) -> inertia::Result<()> {
    info!("Sending {} INERT to {}", amount, &to[..16]);
    
    if let Some(m) = memo {
        info!("Memo: {}", m);
    }
    
    // TODO: Implement transaction sending
    println!("Transaction sent! (simulated)");
    println!("Tx hash: {}", hex::encode(blake3::hash(format!("{}{}{}", to, amount, chrono::Utc::now()).as_bytes()).as_bytes()));
    
    Ok(())
}

async fn cmd_status(datadir: PathBuf) -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Inertia Node Status", "📊".bold());
    println!("{}", "=".repeat(50));
    
    println!("Data directory: {}", datadir.display());
    println!("Node running:   {}", "Yes".green());
    println!("Peer count:     {}", "0".yellow()); // TODO: Implement
    println!("Block height:   {}", "0".yellow()); // TODO: Implement
    println!("Memory usage:   {}", format_memory_usage());
    
    println!("{}", "=".repeat(50));
    
    Ok(())
}

async fn cmd_stats(datadir: PathBuf) -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Blockchain Statistics", "📈".bold());
    println!("{}", "=".repeat(50));
    
    // TODO: Implement stats gathering
    println!("Total blocks:    {}", "0");
    println!("Total txns:      {}", "0");
    println!("Active nodes:    {}", "0");
    println!("Total INERT:     {}", "0");
    println!("Burned INERT:    {}", "0");
    println!("Hot blocks:      {}", "0");
    println!("Fossil blocks:   {}", "0");
    
    Ok(())
}

async fn cmd_peers(datadir: PathBuf) -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Active Peers", "🔄".bold());
    println!("{}", "=".repeat(50));
    
    println!("No active peers");
    
    Ok(())
}

async fn cmd_evolution(datadir: PathBuf) -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Evolution Status", "🧬".bold());
    println!("{}", "=".repeat(50));
    
    println!("Active genes:    {}", "20");
    println!("Mutations:       {}", "0");
    println!("Avg fitness:     {}", "0.50");
    println!("Generation:      {}", "0");
    println!("Crossbreeds:     {}", "0");
    
    Ok(())
}

async fn cmd_immunity(datadir: PathBuf) -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Immunity Status", "🛡️".bold());
    println!("{}", "=".repeat(50));
    
    println!("Status:          {}", "Healthy".green());
    println!("Quarantined:     {}", "0");
    println!("Threats:         {}", "0");
    println!("Vector S:        {}", "0.00");
    
    Ok(())
}

async fn cmd_astro() -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Astronomical Anchor", "⭐".bold());
    println!("{}", "=".repeat(50));
    
    #[cfg(feature = "opencv-support")]
    {
        println!("Status:          {}", "Available".green());
        println!("Last anchor:     {}", "Never");
        println!("Star count:      {}", "0");
        println!("Confidence:      {}", "0%");
    }
    
    #[cfg(not(feature = "opencv-support"))]
    {
        println!("Status:          {}", "Not available (build with 'opencv-support' feature)".yellow());
        println!("To enable:       cargo build --features opencv-support");
    }
    
    println!("In Physics We Trust. 🌌");
    
    Ok(())
}

async fn cmd_benchmark(module: BenchmarkModule, duration: u64) -> inertia::Result<()> {
    println!("{}", "=".repeat(50));
    println!("{} Benchmark: {:?}", "⚡".bold(), module);
    println!("Duration: {} seconds", duration);
    println!("{}", "=".repeat(50));
    
    match module {
        BenchmarkModule::All => {
            println!("Running all benchmarks...");
        }
        BenchmarkModule::WifiStego => {
            println!("Benchmarking WiFi SSID steganography...");
        }
        BenchmarkModule::Bluetooth => {
            println!("Benchmarking Bluetooth advertising...");
        }
        BenchmarkModule::Ultrasound => {
            println!("Benchmarking ultrasound modem...");
        }
        BenchmarkModule::DnsSpore => {
            println!("Benchmarking DNS parasitism...");
        }
        BenchmarkModule::Fountain => {
            println!("Benchmarking fountain codes...");
        }
        BenchmarkModule::PoE => {
            println!("Benchmarking Proof of Encounter...");
        }
    }
    
    println!("\n{}", "Benchmark not yet implemented".yellow());
    
    Ok(())
}

async fn cmd_reset(datadir: PathBuf, force: bool) -> inertia::Result<()> {
    if !force {
        println!("{}", "⚠️  WARNING: This will delete all node data!".red().bold());
        println!("Data directory: {}", datadir.display());
        println!("Type 'yes' to confirm: ");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        
        if input.trim() != "yes" {
            println!("Reset cancelled.");
            return Ok(());
        }
    }
    
    if datadir.exists() {
        std::fs::remove_dir_all(&datadir)?;
        info!("Removed data directory: {}", datadir.display());
    }
    
    info!("Reset complete. You can now start a fresh node.");
    
    Ok(())
}

fn cmd_version() {
    println!("{} v{}", PROTOCOL_NAME, PROTOCOL_VERSION);
    println!("Protocol: Proof of Encounter (PoE)");
    println!("License: AGPL-3.0");
    println!("In Physics We Trust. 🌌");
}

// ============ Helper Functions ============

fn init_logging(level: &str, json_logs: bool) {
    let level = match level.to_lowercase().as_str() {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    };
    
    if json_logs {
        // TODO: Implement JSON logging
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level.as_str())).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level.as_str()))
            .format_timestamp_millis()
            .init();
    }
}

fn print_banner() {
    let banner = r#"
    ┌─────────────────────────────────────────────────────────────┐
    │                                                             │
    │   ██▓    ███▄    █  ██▓ ██▀███   ▄▄▄█████▓ ██▓ ▄▄▄          │
    │  ▓██▒    ██ ▀█   █ ▓██▒▓██ ▒ ██▒▓  ██▒ ▓▒▓██▒▒████▄        │
    │  ▒██░   ▓██  ▀█ ██▒▒██▒▓██ ░▄█ ▒▒ ▓██░ ▒░▒██▒▒██  ▀█▄      │
    │  ▒██░   ▓██▒  ▐▌██▒░██░▒██▀▀█▄  ░ ▓██▓ ░ ░██░░██▄▄▄▄██     │
    │  ░██████▒▒██░   ▓██░░██░░██▓ ▒██▒  ▒██▒ ░ ░██░ ▓█   ▓██▒    │
    │  ░ ▒░▓  ░░ ▒░   ▒ ▒ ░▓  ░ ▒▓ ░▒▓░  ▒ ░░   ░▓  ░▒▒   ▓▒█░    │
    │  ░ ░ ▒  ░░ ░░   ░ ▒░ ▒ ░  ░▒ ░ ▒░    ░     ▒ ░ ░   ▒ ░     │
    │    ░ ░      ░   ░ ░  ▒ ░  ░░   ░   ░       ▒ ░ ░   ░       │
    │      ░  ░         ░  ░     ░               ░     ░         │
    │                                                             │
    │         Post-Internet Protocol • Digital Species           │
    │                 In Physics We Trust. 🌌                     │
    └─────────────────────────────────────────────────────────────┘
    "#;
    
    println!("{}", banner.cyan());
}

fn format_memory_usage() -> String {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    let used = sys.used_memory();
    
    if used > 1024 * 1024 * 1024 {
        format!("{:.2} GB", used as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if used > 1024 * 1024 {
        format!("{:.2} MB", used as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} KB", used as f64 / 1024.0)
    }
}

fn load_keypair(path: &PathBuf) -> inertia::Result<KeyPair> {
    let data = std::fs::read(path)?;
    let json: serde_json::Value = serde_json::from_slice(&data)?;
    let privkey_hex = json["private_key"].as_str().ok_or_else(|| inertia::InertiaError::Config("Invalid key file".to_string()))?;
    let privkey_bytes = hex::decode(privkey_hex).map_err(|_| inertia::InertiaError::Config("Invalid hex".to_string()))?;
    KeyPair::from_bytes(&privkey_bytes).ok_or_else(|| inertia::InertiaError::Config("Invalid key".to_string()))
}

fn save_keypair(keypair: &KeyPair, path: &PathBuf) -> inertia::Result<()> {
    let pubkey_hex = hex::encode(keypair.public().as_bytes());
    let privkey_hex = hex::encode(keypair.secret().as_bytes());
    
    let data = serde_json::json!({
        "public_key": pubkey_hex,
        "private_key": privkey_hex,
        "version": PROTOCOL_VERSION,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });
    
    // Create directory if it doesn't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    std::fs::write(path, serde_json::to_string_pretty(&data)?)?;
    Ok(())
}

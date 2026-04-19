// src/consensus/immunity.rs
// Swarm Immunity — иммунная система для защиты от Eclipse-атак и Sybil
// Inertia Protocol — Post-Internet Digital Species

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex};
use ed25519_dalek::PublicKey;
use log::{debug, info, warn};

const QUARANTINE_THRESHOLD: f64 = 0.3;
const TENSION_THRESHOLD: f64 = 0.7;
const HIBERNATION_TENSION: f64 = -5.0;
const IMMUNE_MEMORY_SIZE: usize = 1000;
const REPUTATION_DECAY_RATE: f64 = 0.99;

#[derive(Debug, Clone, PartialEq)]
pub enum ThreatType {
    Sybil,
    Eclipse,
    Replay,
    Spoof,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ThreatRecord {
    pub threat_type: ThreatType,
    pub detected_at: u64,
    pub source_node: Option<PublicKey>,
    pub tension_value: f64,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Healthy,
    Suspicious,
    Quarantined,
    Hibernating,
}

#[derive(Debug, Clone)]
pub struct SpatialReputation {
    pub node: PublicKey,
    pub position_hash: Option<[u8; 32]>,
    pub reputation_score: f64,
    pub encounter_count: usize,
    pub failed_verifications: usize,
    pub last_seen: u64,
    pub status: NodeStatus,
    pub tension_contributions: VecDeque<f64>,
}

pub struct SwarmImmunity {
    spatial_reputation: Arc<Mutex<HashMap<String, SpatialReputation>>>,
    threat_memory: Arc<Mutex<VecDeque<ThreatRecord>>>,
    tension_vectors: Arc<Mutex<HashMap<String, f64>>>,
    quarantine_threshold: f64,
    tension_threshold: f64,
    reputation_decay_rate: f64,
    self_status: Arc<Mutex<NodeStatus>>,
    hibernation_until: Arc<Mutex<Option<u64>>>,
}

impl SwarmImmunity {
    pub fn new() -> Self {
        Self {
            spatial_reputation: Arc::new(Mutex::new(HashMap::new())),
            threat_memory: Arc::new(Mutex::new(VecDeque::with_capacity(IMMUNE_MEMORY_SIZE))),
            tension_vectors: Arc::new(Mutex::new(HashMap::new())),
            quarantine_threshold: QUARANTINE_THRESHOLD,
            tension_threshold: TENSION_THRESHOLD,
            reputation_decay_rate: REPUTATION_DECAY_RATE,
            self_status: Arc::new(Mutex::new(NodeStatus::Healthy)),
            hibernation_until: Arc::new(Mutex::new(None)),
        }
    }

    pub fn update_reputation(
        &mut self,
        node: &PublicKey,
        position_hash: Option<[u8; 32]>,
        verification_success: bool,
        rssi: i16,
        rtt_us: u32,
    ) -> f64 {
        let node_key = hex::encode(node.as_bytes());
        let timestamp = self.current_timestamp();
        let mut reputation = self.spatial_reputation.lock().unwrap();

        let rep = reputation.entry(node_key).or_insert(SpatialReputation {
            node: *node,
            position_hash,
            reputation_score: 0.5,
            encounter_count: 0,
            failed_verifications: 0,
            last_seen: timestamp,
            status: NodeStatus::Healthy,
            tension_contributions: VecDeque::with_capacity(10),
        });

        if position_hash.is_some() {
            rep.position_hash = position_hash;
        }

        rep.encounter_count += 1;
        rep.last_seen = timestamp;

        if !verification_success {
            rep.failed_verifications += 1;
        }

        let success_rate = (rep.encounter_count - rep.failed_verifications) as f64 / rep.encounter_count as f64;
        let physical_factor = self.calculate_physical_factor(rssi, rtt_us);
        rep.reputation_score = (success_rate * 0.7 + physical_factor * 0.3).clamp(0.0, 1.0);

        rep.status = if rep.reputation_score < self.quarantine_threshold {
            NodeStatus::Quarantined
        } else if rep.reputation_score < self.quarantine_threshold + 0.2 {
            NodeStatus::Suspicious
        } else {
            NodeStatus::Healthy
        };

        let tension_contribution = 1.0 - rep.reputation_score;
        rep.tension_contributions.push_back(tension_contribution);
        if rep.tension_contributions.len() > 10 {
            rep.tension_contributions.pop_front();
        }

        rep.reputation_score
    }

    pub fn calculate_spatial_tension(&self, node: &PublicKey) -> f64 {
        let node_key = hex::encode(node.as_bytes());
        let reputation = self.spatial_reputation.lock().unwrap();

        if let Some(rep) = reputation.get(&node_key) {
            let mut gradient = 0.0;
            let mut neighbor_count = 0;

            for other in reputation.values() {
                if other.node != *node {
                    let proximity = self.calculate_proximity(rep, other);
                    let rep_diff = rep.reputation_score - other.reputation_score;
                    gradient += rep_diff * proximity;
                    neighbor_count += 1;
                }
            }

            if neighbor_count > 0 {
                gradient /= neighbor_count as f64;
            }

            let divergence = if rep.tension_contributions.len() > 1 {
                let mean: f64 = rep.tension_contributions.iter().sum::<f64>() / rep.tension_contributions.len() as f64;
                let variance: f64 = rep.tension_contributions.iter()
                    .map(|&x| (x - mean).powi(2))
                    .sum::<f64>() / rep.tension_contributions.len() as f64;
                variance.sqrt()
            } else {
                0.0
            };

            let tension = gradient * (1.0 - rep.reputation_score) - divergence;
            let mut tensions = self.tension_vectors.lock().unwrap();
            tensions.insert(node_key, tension);
            return tension;
        }
        0.0
    }

    pub fn should_hibernate(&mut self, node: &PublicKey) -> bool {
        let tension = self.calculate_spatial_tension(node);
        if tension < HIBERNATION_TENSION {
            warn!("High negative tension ({:.2}) — entering hibernation", tension);
            let wake_time = self.current_timestamp() + 3600;
            *self.hibernation_until.lock().unwrap() = Some(wake_time);
            *self.self_status.lock().unwrap() = NodeStatus::Hibernating;
            self.record_threat(ThreatType::Eclipse, Some(*node), tension, "High negative spatial tension detected");
            return true;
        }
        false
    }

    pub fn is_hibernating(&self) -> bool {
        if let Some(wake_until) = *self.hibernation_until.lock().unwrap() {
            if self.current_timestamp() < wake_until {
                return true;
            } else {
                let mut status = self.self_status.lock().unwrap();
                *status = NodeStatus::Healthy;
                let mut hiber = self.hibernation_until.lock().unwrap();
                *hiber = None;
                info!("Waking up from hibernation");
            }
        }
        false
    }

    pub fn receive_immune_signal(&mut self, from: &PublicKey, signal_strength: f64) {
        let node_key = hex::encode(from.as_bytes());
        let mut reputation = self.spatial_reputation.lock().unwrap();
        if let Some(rep) = reputation.get_mut(&node_key) {
            rep.reputation_score *= (1.0 - signal_strength * 0.1).max(0.0);
            if rep.reputation_score < self.quarantine_threshold {
                rep.status = NodeStatus::Quarantined;
                info!("Node quarantined due to immune signal");
            }
        }
    }

    pub fn is_node_healthy(&self, node: &PublicKey) -> bool {
        let node_key = hex::encode(node.as_bytes());
        let reputation = self.spatial_reputation.lock().unwrap();
        if let Some(rep) = reputation.get(&node_key) {
            rep.status == NodeStatus::Healthy
        } else {
            true
        }
    }

    pub fn get_node_status(&self, node: &PublicKey) -> NodeStatus {
        let node_key = hex::encode(node.as_bytes());
        let reputation = self.spatial_reputation.lock().unwrap();
        if let Some(rep) = reputation.get(&node_key) {
            rep.status.clone()
        } else {
            NodeStatus::Healthy
        }
    }

    pub fn decay_reputations(&mut self) {
        let mut reputation = self.spatial_reputation.lock().unwrap();
        let now = self.current_timestamp();
        for rep in reputation.values_mut() {
            let hours_since_last = (now - rep.last_seen) as f64 / 3600.0;
            let decay = self.reputation_decay_rate.powf(hours_since_last);
            rep.reputation_score *= decay;
            rep.reputation_score = rep.reputation_score.clamp(0.0, 1.0);
        }
    }

    pub fn get_healthy_neighbors(&self, neighbors: &[PublicKey]) -> Vec<PublicKey> {
        neighbors.iter()
            .filter(|&n| self.is_node_healthy(n))
            .cloned()
            .collect()
    }

    pub fn release_from_quarantine(&mut self, node: &PublicKey) {
        let node_key = hex::encode(node.as_bytes());
        let mut reputation = self.spatial_reputation.lock().unwrap();
        if let Some(rep) = reputation.get_mut(&node_key) {
            rep.status = NodeStatus::Healthy;
            rep.reputation_score = 0.7;
            info!("Node released from quarantine");
        }
    }

    pub fn get_threat_history(&self) -> Vec<ThreatRecord> {
        self.threat_memory.lock().unwrap().iter().cloned().collect()
    }

    fn record_threat(&mut self, threat_type: ThreatType, source: Option<PublicKey>, tension: f64, description: &str) {
        let mut memory = self.threat_memory.lock().unwrap();
        memory.push_front(ThreatRecord {
            threat_type,
            detected_at: self.current_timestamp(),
            source_node: source,
            tension_value: tension,
            description: description.to_string(),
        });
        if memory.len() > IMMUNE_MEMORY_SIZE {
            memory.pop_back();
        }
        warn!("Threat recorded: {:?}", threat_type);
    }

    fn calculate_physical_factor(&self, rssi: i16, rtt_us: u32) -> f64 {
        let rssi_factor = ((rssi + 100) as f64 / 70.0).clamp(0.0, 1.0);
        let rtt_factor = (1.0 - (rtt_us as f64 / 1000.0).min(1.0)).clamp(0.0, 1.0);
        (rssi_factor + rtt_factor) / 2.0
    }

    fn calculate_proximity(&self, a: &SpatialReputation, b: &SpatialReputation) -> f64 {
        if let (Some(pos_a), Some(pos_b)) = (a.position_hash, b.position_hash) {
            let mut diff = 0;
            for i in 0..32 {
                diff += (pos_a[i] as i32 - pos_b[i] as i32).abs() as u32;
            }
            1.0 - (diff as f64 / 256.0).min(1.0)
        } else {
            0.5
        }
    }

    fn current_timestamp(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }
}

impl Default for SwarmImmunity {
    fn default() -> Self {
        Self::new()
    }
}

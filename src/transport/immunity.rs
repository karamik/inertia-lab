// src/consensus/immunity.rs
// Swarm Immunity — иммунная система для защиты от Eclipse-атак и Sybil
// Inertia Protocol — Post-Internet Digital Species
//
// Swarm Immunity — биологический подход к безопасности сети:
// 1. Вектор напряжённости S для детектирования аномалий
// 2. Геометрическая репутация (нельзя подделать физику)
// 3. Иммунный ответ роя (коллективная защита)
// 4. Карантин заражённых узлов
// 5. Самолечение через астро-якоря

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::{Arc, Mutex};
use ed25519_dalek::{PublicKey, Verifier};
use blake3::Hash;
use log::{debug, info, warn, error};

// Константы иммунной системы
const QUARANTINE_THRESHOLD: f64 = 0.3;        // Репутация ниже 0.3 → карантин
const TENSION_THRESHOLD: f64 = 0.7;           // Вектор напряжённости выше 0.7 → тревога
const HIBERNATION_TENSION: f64 = -5.0;        // Отрицательная напряжённость → спячка
const IMMUNE_MEMORY_SIZE: usize = 1000;       // Память об атаках
const REPUTATION_DECAY_RATE: f64 = 0.99;      // Забывание за каждый час
const MIN_HEALTHY_NEIGHBORS: usize = 3;       // Минимум здоровых соседей

/// Тип угрозы для иммунной системы
#[derive(Debug, Clone, PartialEq)]
pub enum ThreatType {
    Sybil,          // Множество фальшивых узлов
    Eclipse,        // Изоляция честного узла
    Replay,         // Повтор старых встреч
    Spoof,          // Подделка радиоэнтропии
    Unknown,
}

/// Запись об атаке в иммунной памяти
#[derive(Debug, Clone)]
pub struct ThreatRecord {
    pub threat_type: ThreatType,
    pub detected_at: u64,
    pub source_node: Option<PublicKey>,
    pub tension_value: f64,
    pub description: String,
}

/// Статус узла в иммунной системе
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Healthy,        // Здоровый узел
    Suspicious,     // Подозрительный (наблюдение)
    Quarantined,    // В карантине (игнорируется)
    Hibernating,    // В спячке (проверяет звёзды)
}

/// Репутация узла в пространстве
#[derive(Debug, Clone)]
pub struct SpatialReputation {
    pub node: PublicKey,
    pub position_hash: Option<[u8; 32]>,     // Хеш позиции (GPS/астро)
    pub reputation_score: f64,               // 0.0 - 1.0
    pub encounter_count: usize,
    pub failed_verifications: usize,
    pub last_seen: u64,
    pub status: NodeStatus,
    pub tension_contributions: VecDeque<f64>, // История вклада в тензор
}

/// Иммунная система роя
pub struct SwarmImmunity {
    // Репутация узлов
    spatial_reputation: Arc<Mutex<HashMap<String, SpatialReputation>>>,
    
    // Память об атаках
    threat_memory: Arc<Mutex<VecDeque<ThreatRecord>>>,
    
    // Текущий вектор напряжённости для каждого узла
    tension_vectors: Arc<Mutex<HashMap<String, f64>>>,
    
    // Параметры иммунной системы
    quarantine_threshold: f64,
    tension_threshold: f64,
    reputation_decay_rate: f64,
    
    // Собственный статус
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
    
    /// Обновление репутации узла на основе встречи
    pub fn update_reputation(
        &mut self,
        node: &PublicKey,
        position_hash: Option<[u8; 32]>,
        verification_success: bool,
        rssi: i16,
        rtt_us: u32,
    ) -> f64 {
        
        let node_key = self.public_key_to_string(node);
        let timestamp = self.current_timestamp();
        
        let mut reputation = self.spatial_reputation.lock().unwrap();
        
        let rep = reputation.entry(node_key).or_insert(SpatialReputation {
            node: *node,
            position_hash,
            reputation_score: 0.5,  // Начальная нейтральная репутация
            encounter_count: 0,
            failed_verifications: 0,
            last_seen: timestamp,
            status: NodeStatus::Healthy,
            tension_contributions: VecDeque::with_capacity(10),
        });
        
        // Обновляем позицию, если есть новые данные
        if position_hash.is_some() {
            rep.position_hash = position_hash;
        }
        
        rep.encounter_count += 1;
        rep.last_seen = timestamp;
        
        if !verification_success {
            rep.failed_verifications += 1;
        }
        
        // Вычисляем новую репутацию на основе успешных встреч
        let success_rate = (rep.encounter_count - rep.failed_verifications) as f64 / rep.encounter_count as f64;
        
        // Физический фактор (RSSI и RTT)
        let physical_factor = self.calculate_physical_factor(rssi, rtt_us);
        
        // Итоговая репутация
        rep.reputation_score = (success_rate * 0.7 + physical_factor * 0.3).clamp(0.0, 1.0);
        
        // Обновляем статус на основе репутации
        rep.status = if rep.reputation_score < self.quarantine_threshold {
            NodeStatus::Quarantined
        } else if rep.reputation_score < self.quarantine_threshold + 0.2 {
            NodeStatus::Suspicious
        } else {
            NodeStatus::Healthy
        };
        
        // Добавляем вклад в тензор напряжённости
        let tension_contribution = self.calculate_tension_contribution(rep);
        rep.tension_contributions.push_back(tension_contribution);
        if rep.tension_contributions.len() > 10 {
            rep.tension_contributions.pop_front();
        }
        
        debug!("Reputation for {}: {:.3} ({:?})", 
               self.public_key_to_short(node), rep.reputation_score, rep.status);
        
        rep.reputation_score
    }
    
    /// Вычисление вектора напряжённости S для узла
    pub fn calculate_spatial_tension(&self, node: &PublicKey) -> f64 {
        let node_key = self.public_key_to_string(node);
        let reputation = self.spatial_reputation.lock().unwrap();
        
        if let Some(rep) = reputation.get(&node_key) {
            // Градиент репутации в пространстве
            let mut gradient = 0.0;
            let mut neighbor_count = 0;
            
            for (_, other) in reputation.iter() {
                if other.node != *node {
                    // Чем ближе позиции, тем больше влияние
                    let proximity = self.calculate_proximity(rep, other);
                    let rep_diff = rep.reputation_score - other.reputation_score;
                    gradient += rep_diff * proximity;
                    neighbor_count += 1;
                }
            }
            
            if neighbor_count > 0 {
                gradient /= neighbor_count as f64;
            }
            
            // Дивергенция репутации (локальная аномалия)
            let divergence = if rep.tension_contributions.len() > 1 {
                let mean: f64 = rep.tension_contributions.iter().sum::<f64>() / rep.tension_contributions.len() as f64;
                let variance: f64 = rep.tension_contributions.iter()
                    .map(|&x| (x - mean).powi(2))
                    .sum::<f64>() / rep.tension_contributions.len() as f64;
                variance.sqrt()
            } else {
                0.0
            };
            
            // Итоговый вектор напряжённости
            // Отрицательное значение означает аномалию (возможная атака)
            let tension = gradient * (1.0 - rep.reputation_score) - divergence;
            
            // Сохраняем тензор
            let mut tensions = self.tension_vectors.lock().unwrap();
            tensions.insert(node_key, tension);
            
            return tension;
        }
        
        0.0
    }
    
    /// Вычисление вклада в тензор напряжённости
    fn calculate_tension_contribution(&self, rep: &SpatialReputation) -> f64 {
        // Чем ниже репутация, тем больше вклад в напряжённость
        1.0 - rep.reputation_score
    }
    
    /// Вычисление физического фактора на основе RSSI и RTT
    fn calculate_physical_factor(&self, rssi: i16, rtt_us: u32) -> f64 {
        // RSSI: от -100 (плохо) до -30 (отлично)
        let rssi_factor = ((rssi + 100) as f64 / 70.0).clamp(0.0, 1.0);
        
        // RTT: от 0 до 1000 мкс
        let rtt_factor = (1.0 - (rtt_us as f64 / 1000.0).min(1.0)).clamp(0.0, 1.0);
        
        (rssi_factor + rtt_factor) / 2.0
    }
    
    /// Вычисление близости двух узлов на основе их позиций
    fn calculate_proximity(&self, a: &SpatialReputation, b: &SpatialReputation) -> f64 {
        if let (Some(pos_a), Some(pos_b)) = (a.position_hash, b.position_hash) {
            // Хеши позиций — не идеально, но для прототипа достаточно
            let diff = self.hash_diff(&pos_a, &pos_b);
            1.0 - (diff as f64 / 256.0).min(1.0)
        } else {
            // Если позиции неизвестны, считаем всех соседями с весом 0.5
            0.5
        }
    }
    
    /// Разница между двумя хешами
    fn hash_diff(&self, a: &[u8; 32], b: &[u8; 32]) -> u32 {
        let mut diff = 0;
        for i in 0..32 {
            diff += (a[i] as i32 - b[i] as i32).abs() as u32;
        }
        diff
    }
    
    /// Проверка, нужно ли перейти в режим гибернации
    pub fn should_hibernate(&mut self, node: &PublicKey) -> bool {
        let tension = self.calculate_spatial_tension(node);
        
        // Сильная отрицательная напряжённость = атака
        if tension < HIBERNATION_TENSION {
            warn!("High negative tension ({:.2}) — entering hibernation", tension);
            
            let hibernation_duration = Duration::from_secs(3600); // 1 час спячки
            let wake_time = self.current_timestamp() + hibernation_duration.as_secs();
            *self.hibernation_until.lock().unwrap() = Some(wake_time);
            *self.self_status.lock().unwrap() = NodeStatus::Hibernating;
            
            // Записываем угрозу в память
            self.record_threat(ThreatType::Eclipse, Some(*node), tension, 
                              "High negative spatial tension detected");
            
            return true;
        }
        
        false
    }
    
    /// Проверка, находится ли узел в спячке
    pub fn is_hibernating(&self) -> bool {
        if let Some(wake_until) = *self.hibernation_until.lock().unwrap() {
            if self.current_timestamp() < wake_until {
                return true;
            } else {
                // Просыпаемся
                let mut status = self.self_status.lock().unwrap();
                *status = NodeStatus::Healthy;
                let mut hiber = self.hibernation_until.lock().unwrap();
                *hiber = None;
                info!("Waking up from hibernation");
            }
        }
        false
    }
    
    /// Иммунный ответ роя: узел получает сигнал от соседей
    pub fn receive_immune_signal(&mut self, from: &PublicKey, signal_strength: f64) {
        let node_key = self.public_key_to_string(from);
        let mut reputation = self.spatial_reputation.lock().unwrap();
        
        if let Some(rep) = reputation.get_mut(&node_key) {
            // Иммунный сигнал снижает репутацию узла-нарушителя
            rep.reputation_score *= (1.0 - signal_strength * 0.1).max(0.0);
            
            if rep.reputation_score < self.quarantine_threshold {
                rep.status = NodeStatus::Quarantined;
                info!("Node {} quarantined due to immune signal", 
                      self.public_key_to_short(from));
            }
        }
    }
    
    /// Запись угрозы в иммунную память
    pub fn record_threat(&mut self, threat_type: ThreatType, source: Option<PublicKey>, 
                         tension: f64, description: &str) {
        let mut memory = self.threat_memory.lock().unwrap();
        
        let record = ThreatRecord {
            threat_type,
            detected_at: self.current_timestamp(),
            source_node: source,
            tension_value: tension,
            description: description.to_string(),
        };
        
        memory.push_front(record);
        if memory.len() > IMMUNE_MEMORY_SIZE {
            memory.pop_back();
        }
        
        warn!("Threat recorded: {:?} - {}", threat_type, description);
    }
    
    /// Получение истории угроз
    pub fn get_threat_history(&self) -> Vec<ThreatRecord> {
        self.threat_memory.lock().unwrap().iter().cloned().collect()
    }
    
    /// Проверка, является ли узел здоровым
    pub fn is_node_healthy(&self, node: &PublicKey) -> bool {
        let node_key = self.public_key_to_string(node);
        let reputation = self.spatial_reputation.lock().unwrap();
        
        if let Some(rep) = reputation.get(&node_key) {
            rep.status == NodeStatus::Healthy
        } else {
            true // Неизвестные узлы считаем здоровыми
        }
    }
    
    /// Получение статуса узла
    pub fn get_node_status(&self, node: &PublicKey) -> NodeStatus {
        let node_key = self.public_key_to_string(node);
        let reputation = self.spatial_reputation.lock().unwrap();
        
        if let Some(rep) = reputation.get(&node_key) {
            rep.status.clone()
        } else {
            NodeStatus::Healthy
        }
    }
    
    /// Обновление репутации всех узлов (забывание)
    pub fn decay_reputations(&mut self) {
        let mut reputation = self.spatial_reputation.lock().unwrap();
        let now = self.current_timestamp();
        
        for rep in reputation.values_mut() {
            let hours_since_last = (now - rep.last_seen) as f64 / 3600.0;
            let decay = self.reputation_decay_rate.powf(hours_since_last);
            rep.reputation_score *= decay;
            rep.reputation_score = rep.reputation_score.clamp(0.0, 1.0);
        }
        
        debug!("Reputations decayed");
    }
    
    /// Получение здоровых соседей (для проверки консенсуса)
    pub fn get_healthy_neighbors(&self, neighbors: &[PublicKey]) -> Vec<PublicKey> {
        neighbors.iter()
            .filter(|&n| self.is_node_healthy(n))
            .cloned()
            .collect()
    }
    
    /// Сброс карантина для узла (после верификации через звёзды)
    pub fn release_from_quarantine(&mut self, node: &PublicKey) {
        let node_key = self.public_key_to_string(node);
        let mut reputation = self.spatial_reputation.lock().unwrap();
        
        if let Some(rep) = reputation.get_mut(&node_key) {
            rep.status = NodeStatus::Healthy;
            rep.reputation_score = 0.7; // Восстановленная репутация
            info!("Node {} released from quarantine", self.public_key_to_short(node));
        }
    }
    
    // ========== Вспомогательные функции ==========
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    
    fn public_key_to_string(&self, key: &PublicKey) -> String {
        hex::encode(key.as_bytes())
    }
    
    fn public_key_to_short(&self, key: &PublicKey) -> String {
        let hex = hex::encode(key.as_bytes());
        format!("{}...{}", &hex[0..8], &hex[hex.len()-8..])
    }
}

/// Интеграция Swarm Immunity с Proof of Encounter
pub struct ImmunePoE {
    immunity: SwarmImmunity,
    poe_reference: Arc<Mutex<dyn PoEReference>>,
}

pub trait PoEReference {
    fn get_encounter_weight(&self, node: &PublicKey) -> f64;
    fn get_recent_encounters(&self, node: &PublicKey) -> Vec<()>; // Упрощённо
}

impl ImmunePoE {
    pub fn new(poe_reference: Arc<Mutex<dyn PoEReference>>) -> Self {
        Self {
            immunity: SwarmImmunity::new(),
            poe_reference,
        }
    }
    
    /// Адаптивный порог консенсуса на основе иммунитета
    pub fn adaptive_consensus_threshold(&self, node: &PublicKey) -> f64 {
        let tension = self.immunity.calculate_spatial_tension(node);
        
        if tension < 0.0 {
            // Под атакой — повышаем порог
            0.8 + (-tension).min(0.2)
        } else {
            // Нормальный режим
            0.5 + tension * 0.3
        }
    }
    
    /// Проверка, можно ли доверять блоку от этого узла
    pub fn can_trust_block(&mut self, producer: &PublicKey) -> bool {
        if self.immunity.is_hibernating() {
            warn!("Node is hibernating — cannot trust any blocks");
            return false;
        }
        
        let status = self.immunity.get_node_status(producer);
        
        match status {
            NodeStatus::Healthy => true,
            NodeStatus::Suspicious => {
                // Проверяем дополнительно через PoE
                let weight = self.poe_reference.lock().unwrap().get_encounter_weight(producer);
                weight > 0.5
            }
            NodeStatus::Quarantined => false,
            NodeStatus::Hibernating => false,
        }
    }
}

impl Default for SwarmImmunity {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;
    use ed25519_dalek::Keypair;
    
    fn generate_keypair() -> Keypair {
        let mut csprng = thread_rng();
        Keypair::generate(&mut csprng)
    }
    
    #[test]
    fn test_reputation_update() {
        let mut immunity = SwarmImmunity::new();
        let keypair = generate_keypair();
        
        let score = immunity.update_reputation(
            &keypair.public,
            None,
            true,   // успешная верификация
            -45,    // RSSI
            500,    // RTT
        );
        
        assert!(score >= 0.0 && score <= 1.0);
        assert!(score > 0.5); // Должна быть выше начальной
    }
    
    #[test]
    fn test_reputation_decay() {
        let mut immunity = SwarmImmunity::new();
        let keypair = generate_keypair();
        
        immunity.update_reputation(&keypair.public, None, true, -45, 500);
        
        let score_before = immunity.get_node_reputation_for_test(&keypair.public);
        
        // Имитируем прошедшее время
        immunity.decay_reputations();
        
        let score_after = immunity.get_node_reputation_for_test(&keypair.public);
        
        assert!(score_after <= score_before);
    }
    
    #[test]
    fn test_spatial_tension() {
        let mut immunity = SwarmImmunity::new();
        let keypair = generate_keypair();
        
        // Регистрируем несколько встреч
        for i in 0..10 {
            let other = generate_keypair();
            let success = i % 2 == 0; // половина успешных, половина нет
            immunity.update_reputation(&other.public, None, success, -50 - i as i16, 500);
        }
        
        let tension = immunity.calculate_spatial_tension(&keypair.public);
        
        // Напряжённость должна быть в разумных пределах
        assert!(tension > -10.0 && tension < 10.0);
    }
    
    #[test]
    fn test_hibernation_trigger() {
        let mut immunity = SwarmImmunity::new();
        let keypair = generate_keypair();
        
        // Создаём очень низкую репутацию (имитация атаки)
        for _ in 0..20 {
            let other = generate_keypair();
            immunity.update_reputation(&other.public, None, false, -90, 5000);
        }
        
        let should_hibernate = immunity.should_hibernate(&keypair.public);
        
        // Должна сработать гибернация
        assert!(should_hibernate);
        assert!(immunity.is_hibernating());
    }
    
    #[test]
    fn test_immune_signal() {
        let mut immunity = SwarmImmunity::new();
        let attacker = generate_keypair();
        
        // Атакующий узел с низкой репутацией
        immunity.update_reputation(&attacker.public, None, false, -80, 2000);
        
        // Иммунный сигнал от соседей
        immunity.receive_immune_signal(&attacker.public, 0.8);
        
        let status = immunity.get_node_status(&attacker.public);
        assert_eq!(status, NodeStatus::Quarantined);
    }
    
    // Вспомогательная функция для тестов
    impl SwarmImmunity {
        fn get_node_reputation_for_test(&self, node: &PublicKey) -> f64 {
            let node_key = hex::encode(node.as_bytes());
            let rep = self.spatial_reputation.lock().unwrap();
            rep.get(&node_key).map(|r| r.reputation_score).unwrap_or(0.0)
        }
    }
}

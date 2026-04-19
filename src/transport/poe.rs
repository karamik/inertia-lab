// src/consensus/poe.rs
// Proof of Encounter — консенсус через физические встречи
// Inertia Protocol — Post-Internet Digital Species
//
// Proof of Encounter — гениальная альтернатива PoW и PoS:
// 1. Требует физического присутствия (нельзя подделать удалённо)
// 2. Не тратит электричество (в отличие от PoW)
// 3. Не требует капитала (в отличие от PoS)
// 4. Естественно масштабируется с популяцией узлов
// 5. Создаёт социальный граф доверия

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::{Arc, Mutex};
use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use blake3::Hash;
use rand::Rng;
use log::{debug, info, warn, error};

// Константы Proof of Encounter
const MAX_ENCOUNTER_HISTORY: usize = 1000;     // Храним последние 1000 встреч
const ENCOUNTER_TTL_SECS: u64 = 86400;         // 24 часа жизни встречи
const MIN_ENCOUNTERS_PER_BLOCK: usize = 3;     // Минимум встреч для блока
const WEIGHT_DECAY_LAMBDA: f64 = 0.00001157;   // e^(-λt), λ = 1/86400 (1 день)
const RADIO_ENTROPY_BITS: usize = 64;          // 64 бита энтропии из радиоэфира

/// Тип встречи между узлами
#[derive(Debug, Clone, PartialEq)]
pub enum EncounterType {
    Direct,      // Прямая встреча (Bluetooth/WiFi Direct)
    Relay,       // Через ретранслятор (чейн встреч)
    Star,        // Астрономическая верификация (звёзды)
}

/// Структура одной встречи
#[derive(Debug, Clone)]
pub struct Encounter {
    pub id: [u8; 32],                       // Уникальный ID встречи (хеш)
    pub node_a: PublicKey,                  // Узел A (инициатор)
    pub node_b: PublicKey,                  // Узел B (участник)
    pub timestamp: u64,                     // Unix timestamp встречи
    pub encounter_type: EncounterType,      // Тип встречи
    pub rssi: i16,                          // Сила сигнала (dBm)
    pub rtt_us: u32,                        // Время кругового пути (микросекунды)
    pub radio_hash: [u8; 32],               // Хеш радиоэфира в момент встречи
    pub geo_hash: Option<[u8; 32]>,         // Хеш GPS/астро-координат
    pub signature_a: Signature,             // Подпись узла A
    pub signature_b: Option<Signature>,     // Подпись узла B (если есть)
    pub weight: f64,                        // Вычисленный вес встречи
}

/// Блок транзакций с Proof of Encounter
#[derive(Debug, Clone)]
pub struct PoEBlock {
    pub hash: [u8; 32],                     // Хеш блока
    pub previous_hash: [u8; 32],            // Хеш предыдущего блока
    pub timestamp: u64,                     // Время создания
    pub transactions: Vec<Transaction>,     // Транзакции
    pub encounters: Vec<Encounter>,         // Встречи, подтверждающие блок
    pub total_weight: f64,                  // Суммарный вес всех встреч
    pub producer: PublicKey,                // Узел, создавший блок
    pub signature: Signature,               // Подпись производителя
}

/// Простая транзакция (для демо)
#[derive(Debug, Clone)]
pub struct Transaction {
    pub hash: [u8; 32],
    pub from: PublicKey,
    pub to: PublicKey,
    pub amount: u64,
    pub timestamp: u64,
    pub signature: Signature,
}

/// Репутация узла (для Swarm Immunity)
#[derive(Debug, Clone)]
pub struct NodeReputation {
    pub node: PublicKey,
    pub total_encounters: usize,
    pub successful_encounters: usize,
    pub failed_encounters: usize,
    pub last_seen: u64,
    pub reputation_score: f64,              // 0.0 - 1.0
    pub is_quarantined: bool,
}

/// Менеджер Proof of Encounter
pub struct ProofOfEncounter {
    keypair: Keypair,
    encounters: Arc<Mutex<VecDeque<Encounter>>>,
    reputation: Arc<Mutex<HashMap<String, NodeReputation>>>,
    blocks: Arc<Mutex<Vec<PoEBlock>>>,
    pending_transactions: Arc<Mutex<Vec<Transaction>>>,
    rng: Arc<Mutex<rand::rngs::ThreadRng>>,
}

impl ProofOfEncounter {
    pub fn new(keypair: Keypair) -> Self {
        Self {
            keypair,
            encounters: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_ENCOUNTER_HISTORY))),
            reputation: Arc::new(Mutex::new(HashMap::new())),
            blocks: Arc::new(Mutex::new(Vec::new())),
            pending_transactions: Arc::new(Mutex::new(Vec::new())),
            rng: Arc::new(Mutex::new(rand::thread_rng())),
        }
    }
    
    /// Регистрация новой встречи с другим узлом
    pub fn register_encounter(
        &mut self,
        other_node: PublicKey,
        encounter_type: EncounterType,
        rssi: i16,
        rtt_us: u32,
        radio_hash: [u8; 32],
        geo_hash: Option<[u8; 32]>,
        other_signature: Option<Signature>,
    ) -> Result<Encounter, String> {
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Создаём встречу
        let mut encounter = Encounter {
            id: [0u8; 32],
            node_a: self.keypair.public,
            node_b: other_node,
            timestamp,
            encounter_type,
            rssi,
            rtt_us,
            radio_hash,
            geo_hash,
            signature_a: Signature::from_bytes(&[0u8; 64]).unwrap(),
            signature_b: other_signature,
            weight: 0.0,
        };
        
        // Подписываем встречу
        let message = self.encode_encounter_message(&encounter);
        encounter.signature_a = self.keypair.sign(&message);
        
        // Вычисляем хеш встречи
        encounter.id = self.hash_encounter(&encounter);
        
        // Вычисляем вес встречи
        encounter.weight = self.calculate_encounter_weight(&encounter);
        
        // Сохраняем встречу
        let mut encounters = self.encounters.lock().unwrap();
        encounters.push_front(encounter.clone());
        if encounters.len() > MAX_ENCOUNTER_HISTORY {
            encounters.pop_back();
        }
        
        // Обновляем репутацию
        self.update_reputation(&other_node, true);
        
        info!("Registered encounter with {} (weight: {:.4}, RSSI: {} dBm)", 
              self.public_key_to_short(&other_node), encounter.weight, rssi);
        
        // Пытаемся создать блок, если накоплено достаточно встреч
        self.try_create_block();
        
        Ok(encounter)
    }
    
    /// Получение подтверждения встречи от другого узла
    pub fn confirm_encounter(&mut self, encounter_id: [u8; 32], signature: Signature) -> Result<(), String> {
        let mut encounters = self.encounters.lock().unwrap();
        
        for enc in encounters.iter_mut() {
            if enc.id == encounter_id {
                enc.signature_b = Some(signature);
                enc.weight = self.calculate_encounter_weight(enc);
                
                info!("Encounter confirmed: weight updated to {:.4}", enc.weight);
                return Ok(());
            }
        }
        
        Err("Encounter not found".to_string())
    }
    
    /// Добавление транзакции в пул ожидания
    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<(), String> {
        // Верифицируем подпись
        let message = self.encode_transaction_message(&transaction);
        transaction.from.verify(&message, &transaction.signature)
            .map_err(|_| "Invalid transaction signature".to_string())?;
        
        let mut pending = self.pending_transactions.lock().unwrap();
        pending.push(transaction);
        
        Ok(())
    }
    
    /// Вычисление веса встречи (ядро консенсуса)
    fn calculate_encounter_weight(&self, encounter: &Encounter) -> f64 {
        let mut weight = 1.0;
        
        // 1. Фактор времени (экспоненциальное затухание)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let age = (now - encounter.timestamp) as f64;
        weight *= (-WEIGHT_DECAY_LAMBDA * age).exp();
        
        // 2. Фактор RSSI (сила сигнала)
        let rssi_factor = ((encounter.rssi + 100) as f64) / 100.0; // -100..0 dBm -> 0..1
        weight *= rssi_factor.clamp(0.2, 1.0);
        
        // 3. Фактор RTT (время кругового пути)
        let rtt_factor = 1.0 - (encounter.rtt_us as f64 / 1000000.0).min(0.5);
        weight *= rtt_factor.clamp(0.5, 1.0);
        
        // 4. Фактор радиоэнтропии (уникальность радиоэфира)
        let entropy_factor = self.calculate_entropy_factor(&encounter.radio_hash);
        weight *= entropy_factor;
        
        // 5. Фактор типа встречи
        weight *= match encounter.encounter_type {
            EncounterType::Direct => 1.0,
            EncounterType::Relay => 0.7,
            EncounterType::Star => 1.2,  // Астро-верификация даёт бонус
        };
        
        // 6. Фактор репутации узла B
        let node_b_key = self.public_key_to_string(&encounter.node_b);
        let reputation = self.reputation.lock().unwrap();
        if let Some(rep) = reputation.get(&node_b_key) {
            weight *= (0.5 + rep.reputation_score * 0.5);
        }
        
        weight
    }
    
    /// Вычисление фактора радиоэнтропии
    fn calculate_entropy_factor(&self, radio_hash: &[u8; 32]) -> f64 {
        // Энтропия измеряется как количество бит, которые сложно предсказать
        let mut entropy_bits = 0;
        let bytes = radio_hash.as_ref();
        
        for &byte in bytes {
            entropy_bits += byte.count_ones();
        }
        
        (entropy_bits as f64 / RADIO_ENTROPY_BITS as f64).clamp(0.5, 1.0)
    }
    
    /// Попытка создать новый блок
    fn try_create_block(&mut self) {
        let encounters = self.encounters.lock().unwrap();
        
        // Нужно достаточно встреч для создания блока
        if encounters.len() < MIN_ENCOUNTERS_PER_BLOCK {
            return;
        }
        
        // Берём последние встречи с высоким весом
        let mut valid_encounters: Vec<Encounter> = encounters
            .iter()
            .filter(|e| e.weight > 0.1 && e.signature_b.is_some())
            .take(MIN_ENCOUNTERS_PER_BLOCK)
            .cloned()
            .collect();
        
        if valid_encounters.len() < MIN_ENCOUNTERS_PER_BLOCK {
            return;
        }
        
        // Берём транзакции из пула
        let mut pending = self.pending_transactions.lock().unwrap();
        let transactions: Vec<Transaction> = pending.drain(..).take(100).collect();
        
        // Создаём блок
        let total_weight: f64 = valid_encounters.iter().map(|e| e.weight).sum();
        
        let previous_hash = self.blocks.lock().unwrap()
            .last()
            .map(|b| b.hash)
            .unwrap_or([0u8; 32]);
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut block = PoEBlock {
            hash: [0u8; 32],
            previous_hash,
            timestamp,
            transactions,
            encounters: valid_encounters,
            total_weight,
            producer: self.keypair.public,
            signature: Signature::from_bytes(&[0u8; 64]).unwrap(),
        };
        
        // Подписываем блок
        let message = self.encode_block_message(&block);
        block.signature = self.keypair.sign(&message);
        block.hash = self.hash_block(&block);
        
        // Сохраняем блок
        self.blocks.lock().unwrap().push(block.clone());
        
        info!("Created new block with {} transactions, weight: {:.4}", 
              block.transactions.len(), block.total_weight);
    }
    
    /// Проверка валидности блока
    pub fn verify_block(&self, block: &PoEBlock) -> bool {
        // Проверка подписи производителя
        let message = self.encode_block_message(block);
        if block.producer.verify(&message, &block.signature).is_err() {
            warn!("Block has invalid producer signature");
            return false;
        }
        
        // Проверка веса встреч
        let total_weight: f64 = block.encounters.iter().map(|e| e.weight).sum();
        if (total_weight - block.total_weight).abs() > 0.001 {
            warn!("Block weight mismatch");
            return false;
        }
        
        // Проверка каждой встречи
        for encounter in &block.encounters {
            if !self.verify_encounter(encounter) {
                warn!("Invalid encounter in block");
                return false;
            }
        }
        
        true
    }
    
    /// Проверка валидности встречи
    fn verify_encounter(&self, encounter: &Encounter) -> bool {
        // Проверка подписи узла A
        let message_a = self.encode_encounter_message(encounter);
        if encounter.node_a.verify(&message_a, &encounter.signature_a).is_err() {
            return false;
        }
        
        // Проверка подписи узла B (если есть)
        if let Some(sig_b) = &encounter.signature_b {
            let message_b = self.encode_encounter_message(encounter);
            if encounter.node_b.verify(&message_b, sig_b).is_err() {
                return false;
            }
        }
        
        // Проверка хеша встречи
        let computed_hash = self.hash_encounter(encounter);
        if computed_hash != encounter.id {
            return false;
        }
        
        true
    }
    
    /// Обновление репутации узла
    fn update_reputation(&mut self, node: &PublicKey, success: bool) {
        let node_key = self.public_key_to_string(node);
        let mut reputation = self.reputation.lock().unwrap();
        
        let rep = reputation.entry(node_key).or_insert(NodeReputation {
            node: *node,
            total_encounters: 0,
            successful_encounters: 0,
            failed_encounters: 0,
            last_seen: 0,
            reputation_score: 0.5,
            is_quarantined: false,
        });
        
        rep.total_encounters += 1;
        if success {
            rep.successful_encounters += 1;
        } else {
            rep.failed_encounters += 1;
        }
        
        // Вычисляем новую репутацию
        let success_rate = rep.successful_encounters as f64 / rep.total_encounters as f64;
        rep.reputation_score = success_rate;
        
        // Карантин при слишком низкой репутации
        if rep.reputation_score < 0.2 && rep.total_encounters > 10 {
            rep.is_quarantined = true;
            warn!("Node {} quarantined due to low reputation", 
                  self.public_key_to_short(node));
        }
        
        rep.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    
    /// Получение текущего веса консенсуса для узла
    pub fn get_consensus_weight(&self, node: &PublicKey) -> f64 {
        let encounters = self.encounters.lock().unwrap();
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let total_weight: f64 = encounters.iter()
            .filter(|e| e.node_b == *node || e.node_a == *node)
            .map(|e| {
                let age = (now - e.timestamp) as f64;
                e.weight * (-WEIGHT_DECAY_LAMBDA * age).exp()
            })
            .sum();
        
        total_weight
    }
    
    /// Получение всех блоков
    pub fn get_blocks(&self) -> Vec<PoEBlock> {
        self.blocks.lock().unwrap().clone()
    }
    
    /// Получение репутации узла
    pub fn get_reputation(&self, node: &PublicKey) -> Option<NodeReputation> {
        let reputation = self.reputation.lock().unwrap();
        reputation.get(&self.public_key_to_string(node)).cloned()
    }
    
    // ========== Вспомогательные функции ==========
    
    fn encode_encounter_message(&self, encounter: &Encounter) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(encounter.node_a.as_bytes());
        data.extend_from_slice(encounter.node_b.as_bytes());
        data.extend_from_slice(&encounter.timestamp.to_le_bytes());
        data.extend_from_slice(&encounter.rssi.to_le_bytes());
        data.extend_from_slice(&encounter.rtt_us.to_le_bytes());
        data.extend_from_slice(&encounter.radio_hash);
        if let Some(geo) = &encounter.geo_hash {
            data.extend_from_slice(geo);
        }
        data
    }
    
    fn encode_transaction_message(&self, tx: &Transaction) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(tx.from.as_bytes());
        data.extend_from_slice(tx.to.as_bytes());
        data.extend_from_slice(&tx.amount.to_le_bytes());
        data.extend_from_slice(&tx.timestamp.to_le_bytes());
        data
    }
    
    fn encode_block_message(&self, block: &PoEBlock) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&block.previous_hash);
        data.extend_from_slice(&block.timestamp.to_le_bytes());
        data.extend_from_slice(block.producer.as_bytes());
        // Упрощённо: добавляем хеши транзакций
        for tx in &block.transactions {
            data.extend_from_slice(&tx.hash);
        }
        data
    }
    
    fn hash_encounter(&self, encounter: &Encounter) -> [u8; 32] {
        let message = self.encode_encounter_message(encounter);
        *blake3::hash(&message).as_bytes()
    }
    
    fn hash_block(&self, block: &PoEBlock) -> [u8; 32] {
        let message = self.encode_block_message(block);
        *blake3::hash(&message).as_bytes()
    }
    
    fn public_key_to_string(&self, key: &PublicKey) -> String {
        hex::encode(key.as_bytes())
    }
    
    fn public_key_to_short(&self, key: &PublicKey) -> String {
        let hex = hex::encode(key.as_bytes());
        format!("{}...{}", &hex[0..8], &hex[hex.len()-8..])
    }
}

/// Proof of Encounter для использования в Swarm Immunity
pub struct PoEWithImmunity {
    poe: ProofOfEncounter,
    vector_s_enabled: bool,
}

impl PoEWithImmunity {
    pub fn new(keypair: Keypair) -> Self {
        Self {
            poe: ProofOfEncounter::new(keypair),
            vector_s_enabled: true,
        }
    }
    
    /// Вычисление вектора напряжённости S для детектирования Eclipse-атак
    pub fn calculate_spatial_tension(&self, node: &PublicKey) -> f64 {
        let reputation = self.poe.get_reputation(node);
        let weight = self.poe.get_consensus_weight(node);
        
        // Если репутация аномально низкая, а вес высокий — возможна атака
        if let Some(rep) = reputation {
            if rep.reputation_score < 0.3 && weight > 10.0 {
                // Вектор напряжённости отрицательный (опасность)
                return -1.0 * (1.0 - rep.reputation_score) * weight;
            }
        }
        
        // Нормальное состояние
        weight * 0.1
    }
    
    /// Проверка, нужно ли переходить в режим гибернации
    pub fn should_hibernate(&self, node: &PublicKey) -> bool {
        let tension = self.calculate_spatial_tension(node);
        tension < -5.0  // Сильная отрицательная напряжённость
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;
    use rand::thread_rng;
    
    fn generate_keypair() -> Keypair {
        let mut csprng = thread_rng();
        Keypair::generate(&mut csprng)
    }
    
    #[test]
    fn test_encounter_registration() {
        let keypair_a = generate_keypair();
        let keypair_b = generate_keypair();
        
        let mut poe = ProofOfEncounter::new(keypair_a);
        
        let radio_hash = [42u8; 32];
        
        let encounter = poe.register_encounter(
            keypair_b.public,
            EncounterType::Direct,
            -45,    // RSSI -45 dBm
            500,    // RTT 500 µs
            radio_hash,
            None,
            None,
        );
        
        assert!(encounter.is_ok());
        let enc = encounter.unwrap();
        assert!(enc.weight > 0.0);
        assert!(enc.weight <= 1.0);
    }
    
    #[test]
    fn test_block_creation() {
        let keypair_a = generate_keypair();
        let keypair_b = generate_keypair();
        let keypair_c = generate_keypair();
        
        let mut poe = ProofOfEncounter::new(keypair_a);
        let radio_hash = [42u8; 32];
        
        // Регистрируем несколько встреч
        for other in [keypair_b.public, keypair_c.public] {
            let enc = poe.register_encounter(
                other,
                EncounterType::Direct,
                -40,
                300,
                radio_hash,
                None,
                None,
            ).unwrap();
            
            // Имитируем подтверждение от другого узла
            let mut message = Vec::new();
            message.extend_from_slice(enc.node_a.as_bytes());
            message.extend_from_slice(enc.node_b.as_bytes());
            message.extend_from_slice(&enc.timestamp.to_le_bytes());
            let sig = keypair_b.sign(&message);
            let _ = poe.confirm_encounter(enc.id, sig);
        }
        
        // Проверяем, что блок создался
        let blocks = poe.get_blocks();
        assert!(blocks.len() > 0);
    }
    
    #[test]
    fn test_reputation_decay() {
        let keypair_a = generate_keypair();
        let keypair_b = generate_keypair();
        
        let mut poe = ProofOfEncounter::new(keypair_a);
        let radio_hash = [42u8; 32];
        
        // Серия успешных встреч
        for _ in 0..10 {
            let enc = poe.register_encounter(
                keypair_b.public,
                EncounterType::Direct,
                -40,
                300,
                radio_hash,
                None,
                None,
            ).unwrap();
            
            let mut message = Vec::new();
            message.extend_from_slice(enc.node_a.as_bytes());
            message.extend_from_slice(enc.node_b.as_bytes());
            message.extend_from_slice(&enc.timestamp.to_le_bytes());
            let sig = keypair_b.sign(&message);
            let _ = poe.confirm_encounter(enc.id, sig);
        }
        
        let rep = poe.get_reputation(&keypair_b.public);
        assert!(rep.is_some());
        let rep = rep.unwrap();
        assert!(rep.reputation_score > 0.8);
        assert!(rep.total_encounters >= 10);
    }
    
    #[test]
    fn test_weight_calculation() {
        let keypair_a = generate_keypair();
        let keypair_b = generate_keypair();
        
        let mut poe = ProofOfEncounter::new(keypair_a);
        let radio_hash = [42u8; 32];
        
        // Хорошая встреча (сильный сигнал, малая задержка)
        let enc_good = poe.register_encounter(
            keypair_b.public,
            EncounterType::Direct,
            -30,    // Хороший RSSI
            100,    // Малая задержка
            radio_hash,
            None,
            None,
        ).unwrap();
        
        // Плохая встреча (слабый сигнал, большая задержка)
        let enc_bad = poe.register_encounter(
            keypair_b.public,
            EncounterType::Relay,
            -80,    // Плохой RSSI
            2000,   // Большая задержка
            radio_hash,
            None,
            None,
        ).unwrap();
        
        assert!(enc_good.weight > enc_bad.weight);
    }
}

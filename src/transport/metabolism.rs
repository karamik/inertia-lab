// src/consensus/metabolism.rs
// Термодинамика данных — остывание блоков и налог на забвение
// Inertia Protocol — Post-Internet Digital Species
//
// Метаболизм данных — биологический подход к управлению памятью:
// 1. Блоки имеют "температуру" (актуальность)
// 2. Горячие блоки активно реплицируются
// 3. Холодные блоки замерзают и удаляются
// 4. Подогрев блоков требует токены (налог на забвение)
// 5. Спам вымирает естественным путём

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::{Arc, Mutex};
use ed25519_dalek::PublicKey;
use log::{debug, info, warn, error};

// Константы термодинамики данных
const HOT_TEMPERATURE_THRESHOLD: f64 = 0.7;      // >0.7 — горячий блок
const WARM_TEMPERATURE_THRESHOLD: f64 = 0.3;     // 0.3-0.7 — тёплый блок
const COLD_TEMPERATURE_THRESHOLD: f64 = 0.1;     // <0.1 — холодный блок (кандидат на удаление)
const METABOLISM_INTERVAL_SECS: u64 = 3600;      // Каждый час
const REWARM_COST_TOKENS: u64 = 10;              // Стоимость подогрева блока
const FOSSILIZATION_AGE_DAYS: u64 = 30;          // Через 30 дней блок становится окаменелостью
const MIN_HOT_BLOCKS: usize = 10;                // Минимум горячих блоков для консенсуса

/// Тип температуры блока
#[derive(Debug, Clone, PartialEq)]
pub enum Temperature {
    Hot,        // Активно используется, реплицируется часто
    Warm,       // Используется, реплицируется редко
    Cold,       // Кандидат на удаление
    Fossil,     // Окаменелость (только хеш)
}

impl Temperature {
    pub fn from_value(value: f64) -> Self {
        if value >= HOT_TEMPERATURE_THRESHOLD {
            Temperature::Hot
        } else if value >= WARM_TEMPERATURE_THRESHOLD {
            Temperature::Warm
        } else if value >= COLD_TEMPERATURE_THRESHOLD {
            Temperature::Cold
        } else {
            Temperature::Fossil
        }
    }
    
    pub fn value(&self) -> f64 {
        match self {
            Temperature::Hot => 1.0,
            Temperature::Warm => 0.5,
            Temperature::Cold => 0.1,
            Temperature::Fossil => 0.0,
        }
    }
}

/// Блок с термодинамическими свойствами
#[derive(Debug, Clone)]
pub struct ThermodynamicBlock {
    pub hash: [u8; 32],
    pub previous_hash: [u8; 32],
    pub timestamp: u64,
    pub temperature: f64,           // Текущая температура (0-1)
    pub last_accessed: u64,         // Последнее обращение
    pub access_count: u64,          // Количество обращений
    pub replication_count: u64,     // Сколько раз был скопирован
    pub reward_paid: u64,           // Сколько токенов заплачено за подогрев
    pub is_fossil: bool,            // Статус окаменелости
}

/// Транзакция подогрева блока
#[derive(Debug, Clone)]
pub struct RewarmTransaction {
    pub block_hash: [u8; 32],
    pub from: PublicKey,
    pub tokens: u64,
    pub timestamp: u64,
    pub signature: [u8; 64],
}

/// Метаболизм данных — управление жизненным циклом блоков
pub struct DataMetabolism {
    blocks: Arc<Mutex<HashMap<[u8; 32], ThermodynamicBlock>>>,
    fossils: Arc<Mutex<HashMap<[u8; 32], [u8; 32]>>>,      // Хеш блока -> хеш окаменелости
    rewarm_queue: Arc<Mutex<VecDeque<RewarmTransaction>>>,
    total_tokens_burned: Arc<Mutex<u64>>,
    metabolism_interval: u64,
    reward_cost: u64,
}

impl DataMetabolism {
    pub fn new() -> Self {
        Self {
            blocks: Arc::new(Mutex::new(HashMap::new())),
            fossils: Arc::new(Mutex::new(HashMap::new())),
            rewarm_queue: Arc::new(Mutex::new(VecDeque::new())),
            total_tokens_burned: Arc::new(Mutex::new(0)),
            metabolism_interval: METABOLISM_INTERVAL_SECS,
            reward_cost: REWARM_COST_TOKENS,
        }
    }
    
    /// Добавление нового блока в систему
    pub fn add_block(&mut self, block_hash: [u8; 32], previous_hash: [u8; 32]) {
        let timestamp = self.current_timestamp();
        
        let block = ThermodynamicBlock {
            hash: block_hash,
            previous_hash,
            timestamp,
            temperature: 0.8,               // Новый блок — горячий
            last_accessed: timestamp,
            access_count: 1,
            replication_count: 1,
            reward_paid: 0,
            is_fossil: false,
        };
        
        self.blocks.lock().unwrap().insert(block_hash, block);
        debug!("Block {} added to metabolism", hex::encode(&block_hash[..4]));
    }
    
    /// Обращение к блоку (повышает температуру)
    pub fn access_block(&mut self, block_hash: [u8; 32]) -> Option<()> {
        let mut blocks = self.blocks.lock().unwrap();
        
        if let Some(block) = blocks.get_mut(&block_hash) {
            let now = self.current_timestamp();
            let time_since_last = (now - block.last_accessed) as f64 / 3600.0;
            
            // Повышение температуры при обращении
            block.temperature += 0.1 * (-time_since_last / 24.0).exp();
            block.temperature = block.temperature.min(1.0);
            
            block.last_accessed = now;
            block.access_count += 1;
            
            debug!("Block {} accessed, temp: {:.3}", hex::encode(&block_hash[..4]), block.temperature);
            Some(())
        } else {
            None
        }
    }
    
    /// Репликация блока (при распространении в сети)
    pub fn replicate_block(&mut self, block_hash: [u8; 32]) -> Option<()> {
        let mut blocks = self.blocks.lock().unwrap();
        
        if let Some(block) = blocks.get_mut(&block_hash) {
            block.replication_count += 1;
            
            // Репликация немного повышает температуру
            block.temperature += 0.05;
            block.temperature = block.temperature.min(1.0);
            
            debug!("Block {} replicated {} times", hex::encode(&block_hash[..4]), block.replication_count);
            Some(())
        } else {
            None
        }
    }
    
    /// Подогрев блока (требует токены)
    pub fn rewarm_block(&mut self, block_hash: [u8; 32], from: PublicKey, signature: [u8; 64]) -> Result<(), String> {
        let mut blocks = self.blocks.lock().unwrap();
        
        if let Some(block) = blocks.get_mut(&block_hash) {
            // Проверяем, что блок не окаменелость
            if block.is_fossil {
                return Err("Cannot rewarm a fossilized block".to_string());
            }
            
            // Подогрев повышает температуру
            block.temperature += 0.3;
            block.temperature = block.temperature.min(1.0);
            block.reward_paid += self.reward_cost;
            
            // Записываем транзакцию
            let transaction = RewarmTransaction {
                block_hash,
                from,
                tokens: self.reward_cost,
                timestamp: self.current_timestamp(),
                signature,
            };
            
            self.rewarm_queue.lock().unwrap().push_back(transaction);
            *self.total_tokens_burned.lock().unwrap() += self.reward_cost;
            
            info!("Block {} rewarmed (temp: {:.3}, total paid: {})", 
                  hex::encode(&block_hash[..4]), block.temperature, block.reward_paid);
            
            Ok(())
        } else {
            Err("Block not found".to_string())
        }
    }
    
    /// Периодический метаболизм — остывание блоков
    pub fn metabolize(&mut self) -> Vec<[u8; 32]> {
        let now = self.current_timestamp();
        let mut blocks = self.blocks.lock().unwrap();
        let mut to_fossilize = Vec::new();
        
        for (hash, block) in blocks.iter_mut() {
            // Вычисляем возраст в днях
            let age_days = (now - block.timestamp) as f64 / 86400.0;
            
            // Естественное остывание (экспоненциальное)
            let cooling = 0.05 * (age_days / 7.0).min(1.0); // Максимум 5% в неделю
            block.temperature -= cooling;
            block.temperature = block.temperature.max(0.0);
            
            // Проверяем, не пора ли в окаменелости
            if age_days >= FOSSILIZATION_AGE_DAYS as f64 && block.temperature < COLD_TEMPERATURE_THRESHOLD {
                to_fossilize.push(*hash);
            }
        }
        
        // Превращаем холодные блоки в окаменелости
        for hash in &to_fossilize {
            if let Some(block) = blocks.remove(hash) {
                self.fossils.lock().unwrap().insert(*hash, block.previous_hash);
                info!("Block {} fossilized after {} days", hex::encode(&hash[..4]), FOSSILIZATION_AGE_DAYS);
            }
        }
        
        to_fossilize
    }
    
    /// Получение текущей температуры блока
    pub fn get_temperature(&self, block_hash: [u8; 32]) -> Option<Temperature> {
        let blocks = self.blocks.lock().unwrap();
        
        if let Some(block) = blocks.get(&block_hash) {
            Some(Temperature::from_value(block.temperature))
        } else if self.fossils.lock().unwrap().contains_key(&block_hash) {
            Some(Temperature::Fossil)
        } else {
            None
        }
    }
    
    /// Получение всех горячих блоков (для репликации)
    pub fn get_hot_blocks(&self) -> Vec<[u8; 32]> {
        let blocks = self.blocks.lock().unwrap();
        
        blocks.iter()
            .filter(|(_, b)| b.temperature >= HOT_TEMPERATURE_THRESHOLD)
            .map(|(h, _)| *h)
            .collect()
    }
    
    /// Получение статистики метаболизма
    pub fn get_metabolism_stats(&self) -> MetabolismStats {
        let blocks = self.blocks.lock().unwrap();
        let fossils = self.fossils.lock().unwrap();
        let total_burned = self.total_tokens_burned.lock().unwrap();
        
        let hot = blocks.values().filter(|b| b.temperature >= HOT_TEMPERATURE_THRESHOLD).count();
        let warm = blocks.values().filter(|b| b.temperature >= WARM_TEMPERATURE_THRESHOLD && b.temperature < HOT_TEMPERATURE_THRESHOLD).count();
        let cold = blocks.values().filter(|b| b.temperature < WARM_TEMPERATURE_THRESHOLD && b.temperature >= COLD_TEMPERATURE_THRESHOLD).count();
        
        MetabolismStats {
            total_blocks: blocks.len(),
            hot_blocks: hot,
            warm_blocks: warm,
            cold_blocks: cold,
            fossil_blocks: fossils.len(),
            total_tokens_burned: *total_burned,
            pending_rewarms: self.rewarm_queue.lock().unwrap().len(),
        }
    }
    
    /// Восстановление окаменелости в блок (требует много токенов)
    pub defossilize(&mut self, block_hash: [u8; 32], from: PublicKey, signature: [u8; 64]) -> Result<(), String> {
        let mut fossils = self.fossils.lock().unwrap();
        
        if let Some(prev_hash) = fossils.remove(&block_hash) {
            // Окаменелость можно восстановить только зная предыдущий блок
            let now = self.current_timestamp();
            
            let block = ThermodynamicBlock {
                hash: block_hash,
                previous_hash: prev_hash,
                timestamp: now,
                temperature: 0.3,  // Начинает как тёплый
                last_accessed: now,
                access_count: 0,
                replication_count: 0,
                reward_paid: self.reward_cost * 10,  // Восстановление дороже
                is_fossil: false,
            };
            
            self.blocks.lock().unwrap().insert(block_hash, block);
            
            // Записываем транзакцию
            let transaction = RewarmTransaction {
                block_hash,
                from,
                tokens: self.reward_cost * 10,
                timestamp: now,
                signature,
            };
            
            self.rewarm_queue.lock().unwrap().push_back(transaction);
            *self.total_tokens_burned.lock().unwrap() += self.reward_cost * 10;
            
            info!("Block {} defossilized (expensive recovery)", hex::encode(&block_hash[..4]));
            Ok(())
        } else {
            Err("Fossil not found".to_string())
        }
    }
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[derive(Debug)]
pub struct MetabolismStats {
    pub total_blocks: usize,
    pub hot_blocks: usize,
    pub warm_blocks: usize,
    pub cold_blocks: usize,
    pub fossil_blocks: usize,
    pub total_tokens_burned: u64,
    pub pending_rewarms: usize,
}

impl Default for DataMetabolism {
    fn default() -> Self {
        Self::new()
    }
}

// ========== evolution.rs ==========
// Эволюционная память — мутации и адаптация протокола
// Inertia Protocol — Post-Internet Digital Species

/// Ген протокола — мутируемое правило
#[derive(Debug, Clone)]
pub struct ProtocolGene {
    pub id: [u8; 16],
    pub name: String,
    pub value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub mutation_rate: f64,
    pub fitness: f64,
    pub generation: u32,
}

/// Мутация — изменение правила
#[derive(Debug, Clone)]
pub struct Mutation {
    pub gene_id: [u8; 16],
    pub old_value: f64,
    pub new_value: f64,
    pub fitness_improvement: f64,
    pub adopted_at: u64,
    pub adopted_by: Vec<PublicKey>,
}

/// Эволюционная память — самообучение протокола
pub struct EvolutionaryMemory {
    genes: Arc<Mutex<HashMap<[u8; 16], ProtocolGene>>>,
    mutations: Arc<Mutex<VecDeque<Mutation>>>,
    fitness_history: Arc<Mutex<Vec<f64>>>,
    current_generation: Arc<Mutex<u32>>,
    mutation_threshold: f64,
}

impl EvolutionaryMemory {
    pub fn new() -> Self {
        let mut memory = Self {
            genes: Arc::new(Mutex::new(HashMap::new())),
            mutations: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            fitness_history: Arc::new(Mutex::new(Vec::new())),
            current_generation: Arc::new(Mutex::new(0)),
            mutation_threshold: 0.7,
        };
        
        // Инициализация генов протокола по умолчанию
        memory.init_default_genes();
        memory
    }
    
    fn init_default_genes(&mut self) {
        let default_genes = vec![
            ("poe_weight_decay", 0.00001157, 0.000001, 0.0001, 0.1),
            ("quarantine_threshold", 0.3, 0.1, 0.5, 0.05),
            ("tension_threshold", 0.7, 0.5, 0.9, 0.05),
            ("reputation_decay", 0.99, 0.95, 1.0, 0.02),
            ("metabolism_interval", 3600.0, 1800.0, 7200.0, 0.1),
            ("hot_threshold", 0.7, 0.5, 0.9, 0.05),
            ("cold_threshold", 0.1, 0.05, 0.3, 0.05),
        ];
        
        let mut genes = self.genes.lock().unwrap();
        
        for (name, default, min, max, rate) in default_genes {
            let id = blake3::hash(name.as_bytes()).as_bytes()[..16].try_into().unwrap();
            
            genes.insert(id, ProtocolGene {
                id,
                name: name.to_string(),
                value: default,
                min_value: min,
                max_value: max,
                mutation_rate: rate,
                fitness: 0.5,
                generation: 0,
            });
        }
        
        info!("Initialized {} protocol genes", genes.len());
    }
    
    /// Попытка мутации гена
    pub fn mutate_gene(&mut self, gene_id: [u8; 16]) -> Option<Mutation> {
        let mut genes = self.genes.lock().unwrap();
        
        if let Some(gene) = genes.get_mut(&gene_id) {
            let old_value = gene.value;
            
            // Генерация мутации
            let mutation_delta = (rand::random::<f64>() * 2.0 - 1.0) * gene.mutation_rate;
            let new_value = (gene.value + mutation_delta).clamp(gene.min_value, gene.max_value);
            
            gene.value = new_value;
            gene.generation += 1;
            
            let mutation = Mutation {
                gene_id,
                old_value,
                new_value,
                fitness_improvement: 0.0,
                adopted_at: self.current_timestamp(),
                adopted_by: Vec::new(),
            };
            
            let mut mutations = self.mutations.lock().unwrap();
            mutations.push_front(mutation.clone());
            if mutations.len() > 1000 {
                mutations.pop_back();
            }
            
            debug!("Gene {} mutated: {:.4} -> {:.4}", gene.name, old_value, new_value);
            Some(mutation)
        } else {
            None
        }
    }
    
    /// Оценка приспособленности гена
    pub fn evaluate_fitness(&mut self, gene_id: [u8; 16], success_rate: f64) -> f64 {
        let mut genes = self.genes.lock().unwrap();
        
        if let Some(gene) = genes.get_mut(&gene_id) {
            // Обновляем приспособленность на основе успеха
            gene.fitness = gene.fitness * 0.7 + success_rate * 0.3;
            
            // Если приспособленность высокая, распространяем ген
            if gene.fitness > self.mutation_threshold {
                info!("Gene {} is fit (fitness: {:.3})", gene.name, gene.fitness);
            }
            
            gene.fitness
        } else {
            0.0
        }
    }
    
    /// Распространение успешной мутации на другие узлы
    pub fn spread_mutation(&mut self, mutation: Mutation, adopter: PublicKey) -> bool {
        let mut genes = self.genes.lock().unwrap();
        
        if let Some(gene) = genes.get_mut(&mutation.gene_id) {
            // Если мутация улучшает приспособленность, распространяем
            if mutation.fitness_improvement > 0.1 {
                gene.value = mutation.new_value;
                
                let mut mutations = self.mutations.lock().unwrap();
                if let Some(existing) = mutations.iter_mut().find(|m| m.gene_id == mutation.gene_id) {
                    existing.adopted_by.push(adopter);
                }
                
                debug!("Mutation of gene {} spread to new node", gene.name);
                return true;
            }
        }
        
        false
    }
    
    /// Получение текущего значения гена
    pub fn get_gene_value(&self, gene_name: &str) -> Option<f64> {
        let id = blake3::hash(gene_name.as_bytes()).as_bytes()[..16].try_into().ok()?;
        let genes = self.genes.lock().unwrap();
        genes.get(&id).map(|g| g.value)
    }
    
    /// Получение всех генов
    pub fn get_all_genes(&self) -> Vec<ProtocolGene> {
        self.genes.lock().unwrap().values().cloned().collect()
    }
    
    /// Запись успешной эволюции в историю
    pub fn record_evolution_success(&mut self, fitness_gain: f64) {
        let mut history = self.fitness_history.lock().unwrap();
        history.push(fitness_gain);
        
        if history.len() > 100 {
            history.remove(0);
        }
        
        let avg_fitness: f64 = history.iter().sum();
        info!("Evolution recorded: avg fitness {:.3}", avg_fitness);
    }
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Интеграция метаболизма и эволюции в единую систему жизнеобеспечения
pub struct LifeSupport {
    metabolism: DataMetabolism,
    evolution: EvolutionaryMemory,
    last_metabolism_run: u64,
}

impl LifeSupport {
    pub fn new() -> Self {
        Self {
            metabolism: DataMetabolism::new(),
            evolution: EvolutionaryMemory::new(),
            last_metabolism_run: 0,
        }
    }
    
    /// Цикл жизнеобеспечения (запускать периодически)
    pub fn lifecycle_cycle(&mut self) {
        let now = self.current_timestamp();
        
        // Метаболизм раз в час
        if now - self.last_metabolism_run >= 3600 {
            let fossilized = self.metabolism.metabolize();
            
            if !fossilized.is_empty() {
                info!("Metabolized {} fossilized blocks", fossilized.len());
            }
            
            self.last_metabolism_run = now;
        }
        
        // Проверка эволюционной приспособленности
        let stats = self.metabolism.get_metabolism_stats();
        let health_score = (stats.hot_blocks as f64 / stats.total_blocks.max(1) as f64).min(1.0);
        
        // Адаптация на основе здоровья сети
        for gene in self.evolution.get_all_genes() {
            self.evolution.evaluate_fitness(gene.id, health_score);
            
            // Случайные мутации для улучшения
            if rand::random::<f64>() < 0.01 {
                self.evolution.mutate_gene(gene.id);
            }
        }
    }
    
    /// Получение статуса жизнеобеспечения
    pub fn get_life_status(&self) -> LifeStatus {
        let stats = self.metabolism.get_metabolism_stats();
        let genes = self.evolution.get_all_genes();
        
        LifeStatus {
            total_blocks: stats.total_blocks,
            hot_blocks: stats.hot_blocks,
            fossil_blocks: stats.fossil_blocks,
            tokens_burned: stats.total_tokens_burned,
            active_genes: genes.len(),
            network_health: (stats.hot_blocks as f64 / stats.total_blocks.max(1) as f64),
        }
    }
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[derive(Debug)]
pub struct LifeStatus {
    pub total_blocks: usize,
    pub hot_blocks: usize,
    pub fossil_blocks: usize,
    pub tokens_burned: u64,
    pub active_genes: usize,
    pub network_health: f64,
}

impl Default for EvolutionaryMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for LifeSupport {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_temperature_evolution() {
        let mut metabolism = DataMetabolism::new();
        let block_hash = [1u8; 32];
        
        metabolism.add_block(block_hash, [0u8; 32]);
        
        let temp = metabolism.get_temperature(block_hash);
        assert_eq!(temp, Some(Temperature::Hot));
        
        // Доступ повышает температуру
        metabolism.access_block(block_hash);
        assert!(metabolism.get_temperature(block_hash).unwrap() != Temperature::Cold);
    }
    
    #[test]
    fn test_metabolism_cooling() {
        let mut metabolism = DataMetabolism::new();
        let block_hash = [1u8; 32];
        
        metabolism.add_block(block_hash, [0u8; 32]);
        
        // Симулируем много времени
        for _ in 0..100 {
            metabolism.metabolize();
        }
        
        let temp = metabolism.get_temperature(block_hash);
        assert_eq!(temp, Some(Temperature::Fossil));
    }
    
    #[test]
    fn test_gene_mutation() {
        let mut evolution = EvolutionaryMemory::new();
        
        let genes = evolution.get_all_genes();
        assert!(!genes.is_empty());
        
        if let Some(gene) = genes.first() {
            let mutation = evolution.mutate_gene(gene.id);
            assert!(mutation.is_some());
            
            let new_value = evolution.get_gene_value(&gene.name);
            assert!(new_value.is_some());
            assert_ne!(new_value.unwrap(), gene.value);
        }
    }
    
    #[test]
    fn test_fitness_evaluation() {
        let mut evolution = EvolutionaryMemory::new();
        
        let genes = evolution.get_all_genes();
        let gene = genes.first().unwrap();
        
        let fitness = evolution.evaluate_fitness(gene.id, 0.8);
        assert!(fitness > 0.0 && fitness <= 1.0);
    }
    
    #[test]
    fn test_life_support_cycle() {
        let mut life = LifeSupport::new();
        
        let block_hash = [1u8; 32];
        life.metabolism.add_block(block_hash, [0u8; 32]);
        
        life.lifecycle_cycle();
        
        let status = life.get_life_status();
        assert!(status.total_blocks > 0);
        assert!(status.network_health >= 0.0);
    }
}

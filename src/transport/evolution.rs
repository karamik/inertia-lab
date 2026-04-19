// src/consensus/evolution.rs
// Эволюционная память — мутации, естественный отбор и адаптация протокола
// Inertia Protocol — Post-Internet Digital Species
//
// Эволюция — ключевое отличие Inertia от всех других протоколов:
// 1. Протокол мутирует и адаптируется к условиям среды
// 2. Успешные мутации распространяются через сеть
// 3. Неудачные мутации умирают естественной смертью
// 4. Разные популяции могут иметь разные эволюционные ветки
// 5. При встрече ветки скрещиваются (обмен успешными мутациями)

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::{Arc, Mutex};
use ed25519_dalek::PublicKey;
use blake3::Hash;
use rand::Rng;
use log::{debug, info, warn, error};

// Константы эволюционной памяти
const MAX_MUTATION_HISTORY: usize = 1000;       // Храним последние 1000 мутаций
const MUTATION_SPREAD_THRESHOLD: f64 = 0.65;    // Приспособленность >0.65 → распространять
const MUTATION_DEATH_THRESHOLD: f64 = 0.3;      // Приспособленность <0.3 → мутация умирает
const CROSSBREEDING_GENES: usize = 3;           // Количество генов для скрещивания
const EVOLUTION_CYCLE_SECS: u64 = 86400;        // Эволюционный цикл раз в сутки
const MUTATION_PROBABILITY: f64 = 0.01;         // 1% шанс мутации за цикл

/// Тип эволюционного события
#[derive(Debug, Clone, PartialEq)]
pub enum EvolutionEventType {
    Mutation,           // Появление новой мутации
    Spread,             // Распространение мутации
    Death,              // Смерть мутации
    Crossbreed,         // Скрещивание двух веток
    Fixation,           // Мутация зафиксировалась в популяции
}

/// Событие эволюции
#[derive(Debug, Clone)]
pub struct EvolutionEvent {
    pub event_type: EvolutionEventType,
    pub gene_id: [u8; 16],
    pub gene_name: String,
    pub old_value: f64,
    pub new_value: f64,
    pub fitness: f64,
    pub population_size: usize,
    pub timestamp: u64,
}

/// Ген протокола — мутируемое правило
#[derive(Debug, Clone)]
pub struct ProtocolGene {
    pub id: [u8; 16],                       // Уникальный ID гена
    pub name: String,                        // Название (например, "poe_weight_decay")
    pub value: f64,                          // Текущее значение
    pub min_value: f64,                      // Минимальное допустимое значение
    pub max_value: f64,                      // Максимальное допустимое значение
    pub mutation_rate: f64,                  // Скорость мутации (0-1)
    pub fitness: f64,                        // Текущая приспособленность (0-1)
    pub generation: u32,                     // Поколение
    pub parent_id: Option<[u8; 16]>,         // ID родительского гена
    pub children_ids: Vec<[u8; 16]>,         // ID дочерних генов
    pub population_fraction: f64,            // Доля популяции, использующая этот ген
    pub last_updated: u64,                   // Время последнего обновления
}

/// Популяция узлов, использующих определённую версию гена
#[derive(Debug, Clone)]
pub struct GenePopulation {
    pub gene_id: [u8; 16],
    pub nodes: Vec<PublicKey>,
    pub average_fitness: f64,
    pub size: usize,
}

/// Эволюционная память — самообучение протокола
pub struct EvolutionaryMemory {
    // Гены протокола
    genes: Arc<Mutex<HashMap<[u8; 16], ProtocolGene>>>,
    
    // Популяции для каждого гена
    populations: Arc<Mutex<HashMap<[u8; 16], GenePopulation>>>,
    
    // История мутаций
    mutation_history: Arc<Mutex<VecDeque<EvolutionEvent>>>,
    
    // История приспособленности
    fitness_history: Arc<Mutex<Vec<f64>>>,
    
    // Текущее поколение
    current_generation: Arc<Mutex<u32>>,
    
    // Порог распространения мутаций
    spread_threshold: f64,
    death_threshold: f64,
    
    // RNG для мутаций
    rng: Arc<Mutex<rand::rngs::ThreadRng>>,
}

impl EvolutionaryMemory {
    pub fn new() -> Self {
        let mut memory = Self {
            genes: Arc::new(Mutex::new(HashMap::new())),
            populations: Arc::new(Mutex::new(HashMap::new())),
            mutation_history: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_MUTATION_HISTORY))),
            fitness_history: Arc::new(Mutex::new(Vec::new())),
            current_generation: Arc::new(Mutex::new(0)),
            spread_threshold: MUTATION_SPREAD_THRESHOLD,
            death_threshold: MUTATION_DEATH_THRESHOLD,
            rng: Arc::new(Mutex::new(rand::thread_rng())),
        };
        
        // Инициализация генов протокола по умолчанию
        memory.init_default_genes();
        memory
    }
    
    /// Инициализация генов по умолчанию
    fn init_default_genes(&mut self) {
        let default_genes: Vec<(&str, f64, f64, f64, f64)> = vec![
            // Консенсус
            ("poe_weight_decay", 0.00001157, 0.000001, 0.0001, 0.1),
            ("min_encounters_per_block", 3.0, 1.0, 10.0, 0.1),
            ("encounter_ttl_hours", 24.0, 6.0, 168.0, 0.1),
            
            // Иммунитет
            ("quarantine_threshold", 0.3, 0.1, 0.5, 0.05),
            ("tension_threshold", 0.7, 0.5, 0.9, 0.05),
            ("reputation_decay_rate", 0.99, 0.95, 1.0, 0.02),
            ("hibernation_tension", -5.0, -10.0, -1.0, 0.1),
            
            // Метаболизм
            ("hot_threshold", 0.7, 0.5, 0.9, 0.05),
            ("cold_threshold", 0.1, 0.05, 0.3, 0.05),
            ("metabolism_interval_hours", 1.0, 0.5, 24.0, 0.1),
            ("fossilization_age_days", 30.0, 7.0, 365.0, 0.1),
            ("rewarm_cost", 10.0, 1.0, 100.0, 0.1),
            
            // Эволюция
            ("mutation_rate", 0.01, 0.001, 0.1, 0.1),
            ("spread_threshold", 0.65, 0.5, 0.9, 0.05),
            ("death_threshold", 0.3, 0.1, 0.5, 0.05),
            
            // Транспорт
            ("ultrasound_freq_khz", 19.0, 18.0, 22.0, 0.05),
            ("bt_adv_interval_ms", 100.0, 20.0, 500.0, 0.1),
            ("ssid_rotation_secs", 5.0, 1.0, 30.0, 0.1),
            
            // Астрономия
            ("star_confidence_threshold", 0.7, 0.5, 0.95, 0.05),
            ("astro_tolerance_secs", 300.0, 60.0, 3600.0, 0.1),
        ];
        
        let mut genes = self.genes.lock().unwrap();
        let timestamp = self.current_timestamp();
        
        for (name, default_val, min_val, max_val, mutation_rate) in default_genes {
            let id = *blake3::hash(name.as_bytes()).as_bytes();
            let gene_id: [u8; 16] = id[..16].try_into().unwrap();
            
            genes.insert(gene_id, ProtocolGene {
                id: gene_id,
                name: name.to_string(),
                value: default_val,
                min_value: min_val,
                max_value: max_val,
                mutation_rate,
                fitness: 0.5,
                generation: 0,
                parent_id: None,
                children_ids: Vec::new(),
                population_fraction: 1.0,
                last_updated: timestamp,
            });
            
            // Инициализируем популяцию
            self.populations.lock().unwrap().insert(gene_id, GenePopulation {
                gene_id,
                nodes: Vec::new(),
                average_fitness: 0.5,
                size: 0,
            });
        }
        
        info!("Initialized {} protocol genes", genes.len());
    }
    
    /// Попытка мутации гена
    pub fn mutate_gene(&mut self, gene_id: [u8; 16]) -> Option<EvolutionEvent> {
        let mut rng = self.rng.lock().unwrap();
        let mut genes = self.genes.lock().unwrap();
        
        if let Some(gene) = genes.get_mut(&gene_id) {
            // Проверяем вероятность мутации
            if rng.gen_bool(gene.mutation_rate) {
                let old_value = gene.value;
                
                // Генерация мутации (нормальное распределение вокруг текущего значения)
                let mutation_delta: f64 = rng.gen_range(-1.0..1.0) * gene.mutation_rate * 0.5;
                let mut new_value = gene.value + mutation_delta;
                
                // Ограничиваем диапазоном
                new_value = new_value.clamp(gene.min_value, gene.max_value);
                
                if (new_value - old_value).abs() > 0.0001 {
                    gene.value = new_value;
                    gene.generation += 1;
                    gene.last_updated = self.current_timestamp();
                    
                    let event = EvolutionEvent {
                        event_type: EvolutionEventType::Mutation,
                        gene_id,
                        gene_name: gene.name.clone(),
                        old_value,
                        new_value,
                        fitness: gene.fitness,
                        population_size: 1,
                        timestamp: self.current_timestamp(),
                    };
                    
                    self.record_event(event.clone());
                    
                    debug!("Gene {} mutated: {:.6} -> {:.6} (fitness: {:.3})", 
                           gene.name, old_value, new_value, gene.fitness);
                    
                    return Some(event);
                }
            }
        }
        
        None
    }
    
    /// Оценка приспособленности гена на основе успеха в сети
    pub fn evaluate_fitness(&mut self, gene_id: [u8; 16], success_rate: f64) -> f64 {
        let mut genes = self.genes.lock().unwrap();
        let mut populations = self.populations.lock().unwrap();
        
        if let Some(gene) = genes.get_mut(&gene_id) {
            // Обновляем приспособленность (экспоненциальное скользящее среднее)
            let alpha = 0.3;
            let new_fitness = gene.fitness * (1.0 - alpha) + success_rate * alpha;
            gene.fitness = new_fitness.clamp(0.0, 1.0);
            gene.last_updated = self.current_timestamp();
            
            // Обновляем среднюю приспособленность популяции
            if let Some(pop) = populations.get_mut(&gene_id) {
                pop.average_fitness = (pop.average_fitness * 0.7 + new_fitness * 0.3).clamp(0.0, 1.0);
            }
            
            // Записываем в историю
            let mut history = self.fitness_history.lock().unwrap();
            history.push(new_fitness);
            if history.len() > 1000 {
                history.remove(0);
            }
            
            // Проверяем, нужно ли распространять мутацию
            if new_fitness > self.spread_threshold {
                self.try_spread_mutation(gene_id, new_fitness);
            }
            
            // Проверяем, не умерла ли мутация
            if new_fitness < self.death_threshold && gene.generation > 0 {
                self.kill_mutation(gene_id);
            }
            
            new_fitness
        } else {
            0.0
        }
    }
    
    /// Попытка распространения успешной мутации
    fn try_spread_mutation(&mut self, gene_id: [u8; 16], fitness: f64) {
        let mut genes = self.genes.lock().unwrap();
        
        if let Some(gene) = genes.get_mut(&gene_id) {
            // Увеличиваем долю популяции
            let spread_factor = (fitness - self.spread_threshold) / (1.0 - self.spread_threshold);
            gene.population_fraction = (gene.population_fraction + spread_factor * 0.1).min(1.0);
            
            let event = EvolutionEvent {
                event_type: EvolutionEventType::Spread,
                gene_id,
                gene_name: gene.name.clone(),
                old_value: gene.value,
                new_value: gene.value,
                fitness,
                population_size: (gene.population_fraction * 100.0) as usize,
                timestamp: self.current_timestamp(),
            };
            
            self.record_event(event);
            
            info!("Gene {} spreading (fitness: {:.3}, pop: {:.1}%)", 
                  gene.name, fitness, gene.population_fraction * 100.0);
        }
    }
    
    /// Смерть неудачной мутации
    fn kill_mutation(&mut self, gene_id: [u8; 16]) {
        let mut genes = self.genes.lock().unwrap();
        
        if let Some(gene) = genes.get(&gene_id) {
            let event = EvolutionEvent {
                event_type: EvolutionEventType::Death,
                gene_id,
                gene_name: gene.name.clone(),
                old_value: gene.value,
                new_value: gene.value,
                fitness: gene.fitness,
                population_size: 0,
                timestamp: self.current_timestamp(),
            };
            
            self.record_event(event);
            
            warn!("Gene {} died (fitness: {:.3})", gene.name, gene.fitness);
            
            // Возвращаемся к родительскому гену, если есть
            if let Some(parent_id) = gene.parent_id {
                if let Some(parent) = genes.get(&parent_id) {
                    info!("Reverting to parent gene {}", parent.name);
                }
            }
        }
    }
    
    /// Скрещивание двух эволюционных веток
    pub fn crossbreed(&mut self, gene_a_id: [u8; 16], gene_b_id: [u8; 16]) -> Option<[u8; 16]> {
        let genes = self.genes.lock().unwrap();
        
        let gene_a = genes.get(&gene_a_id)?;
        let gene_b = genes.get(&gene_b_id)?;
        
        if gene_a.name != gene_b.name {
            return None;
        }
        
        // Создаём гибридный ген (среднее арифметическое с весами по приспособленности)
        let total_fitness = gene_a.fitness + gene_b.fitness;
        let weight_a = gene_a.fitness / total_fitness;
        let weight_b = gene_b.fitness / total_fitness;
        
        let hybrid_value = gene_a.value * weight_a + gene_b.value * weight_b;
        let hybrid_value = hybrid_value.clamp(gene_a.min_value, gene_a.max_value);
        
        // Создаём новый ген
        let hybrid_id = self.create_child_gene(&gene_a, hybrid_value);
        
        let event = EvolutionEvent {
            event_type: EvolutionEventType::Crossbreed,
            gene_id: hybrid_id,
            gene_name: gene_a.name.clone(),
            old_value: 0.0,
            new_value: hybrid_value,
            fitness: (gene_a.fitness + gene_b.fitness) / 2.0,
            population_size: 1,
            timestamp: self.current_timestamp(),
        };
        
        self.record_event(event);
        
        info!("Crossbreed: {} ({:.3}) + {} ({:.3}) -> hybrid ({:.3})",
              gene_a.name, gene_a.fitness, gene_b.name, gene_b.fitness, hybrid_value);
        
        Some(hybrid_id)
    }
    
    /// Создание дочернего гена
    fn create_child_gene(&mut self, parent: &ProtocolGene, value: f64) -> [u8; 16] {
        let mut genes = self.genes.lock().unwrap();
        let mut rng = self.rng.lock().unwrap();
        
        let child_id: [u8; 16] = rng.gen();
        
        let child = ProtocolGene {
            id: child_id,
            name: parent.name.clone(),
            value,
            min_value: parent.min_value,
            max_value: parent.max_value,
            mutation_rate: parent.mutation_rate,
            fitness: parent.fitness * 0.8,  // Начинает с чуть меньшей приспособленностью
            generation: parent.generation + 1,
            parent_id: Some(parent.id),
            children_ids: Vec::new(),
            population_fraction: 0.01,       // Начинает с малой доли
            last_updated: self.current_timestamp(),
        };
        
        genes.insert(child_id, child);
        
        // Обновляем родителя
        if let Some(parent_gene) = genes.get_mut(&parent.id) {
            parent_gene.children_ids.push(child_id);
        }
        
        // Создаём популяцию для ребёнка
        self.populations.lock().unwrap().insert(child_id, GenePopulation {
            gene_id: child_id,
            nodes: Vec::new(),
            average_fitness: 0.4,
            size: 0,
        });
        
        child_id
    }
    
    /// Регистрация узла, использующего определённую версию гена
    pub fn register_node(&mut self, gene_id: [u8; 16], node: PublicKey) {
        let mut populations = self.populations.lock().unwrap();
        
        if let Some(pop) = populations.get_mut(&gene_id) {
            if !pop.nodes.contains(&node) {
                pop.nodes.push(node);
                pop.size = pop.nodes.len();
            }
        }
    }
    
    /// Получение текущего значения гена
    pub fn get_gene_value(&self, gene_name: &str) -> Option<f64> {
        let id = blake3::hash(gene_name.as_bytes()).as_bytes();
        let gene_id: [u8; 16] = id[..16].try_into().ok()?;
        let genes = self.genes.lock().unwrap();
        genes.get(&gene_id).map(|g| g.value)
    }
    
    /// Получение значения гена с учётом эволюции (самый приспособленный вариант)
    pub fn get_best_gene_value(&self, gene_name: &str) -> Option<f64> {
        let genes = self.genes.lock().unwrap();
        
        // Ищем все гены с этим именем
        let mut best_gene: Option<&ProtocolGene> = None;
        
        for gene in genes.values() {
            if gene.name == gene_name {
                if best_gene.is_none() || gene.fitness > best_gene.unwrap().fitness {
                    best_gene = Some(gene);
                }
            }
        }
        
        best_gene.map(|g| g.value)
    }
    
    /// Получение всех генов
    pub fn get_all_genes(&self) -> Vec<ProtocolGene> {
        self.genes.lock().unwrap().values().cloned().collect()
    }
    
    /// Получение активных мутаций (с высокой приспособленностью)
    pub fn get_active_mutations(&self) -> Vec<ProtocolGene> {
        self.genes.lock().unwrap()
            .values()
            .filter(|g| g.fitness > self.spread_threshold && g.population_fraction > 0.1)
            .cloned()
            .collect()
    }
    
    /// Запись события эволюции
    fn record_event(&mut self, event: EvolutionEvent) {
        let mut history = self.mutation_history.lock().unwrap();
        history.push_front(event);
        
        if history.len() > MAX_MUTATION_HISTORY {
            history.pop_back();
        }
    }
    
    /// Получение истории эволюции
    pub fn get_evolution_history(&self) -> Vec<EvolutionEvent> {
        self.mutation_history.lock().unwrap().iter().cloned().collect()
    }
    
    /// Получение средней приспособленности сети
    pub fn get_average_fitness(&self) -> f64 {
        let history = self.fitness_history.lock().unwrap();
        
        if history.is_empty() {
            0.5
        } else {
            history.iter().sum::<f64>() / history.len() as f64
        }
    }
    
    /// Эволюционный цикл (запускать периодически)
    pub fn evolution_cycle(&mut self) -> Vec<EvolutionEvent> {
        let mut events = Vec::new();
        let genes_to_mutate: Vec<[u8; 16]> = self.genes.lock().unwrap()
            .keys()
            .cloned()
            .collect();
        
        for gene_id in genes_to_mutate {
            if let Some(event) = self.mutate_gene(gene_id) {
                events.push(event);
            }
        }
        
        // Обновляем поколение
        let mut generation = self.current_generation.lock().unwrap();
        *generation += 1;
        
        if !events.is_empty() {
            info!("Evolution cycle complete: {} mutations", events.len());
        }
        
        events
    }
    
    /// Синхронизация эволюционного состояния с другим узлом
    pub fn sync_with_node(&mut self, remote_genes: Vec<ProtocolGene>, node: PublicKey) -> usize {
        let mut synced = 0;
        let mut local_genes = self.genes.lock().unwrap();
        
        for remote_gene in remote_genes {
            if let Some(local_gene) = local_genes.get_mut(&remote_gene.id) {
                // Если у удалённого узла более приспособленный ген, заимствуем
                if remote_gene.fitness > local_gene.fitness + 0.1 {
                    local_gene.value = remote_gene.value;
                    local_gene.fitness = (local_gene.fitness + remote_gene.fitness) / 2.0;
                    local_gene.population_fraction = (local_gene.population_fraction + 1.0) / 2.0;
                    synced += 1;
                    
                    debug!("Synced gene {} from node", remote_gene.name);
                }
            }
        }
        
        synced
    }
    
    /// Сброс эволюции (для тестов)
    pub fn reset(&mut self) {
        self.genes.lock().unwrap().clear();
        self.populations.lock().unwrap().clear();
        self.mutation_history.lock().unwrap().clear();
        self.fitness_history.lock().unwrap().clear();
        *self.current_generation.lock().unwrap() = 0;
        self.init_default_genes();
        
        info!("Evolutionary memory reset");
    }
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Эволюционный движок — интеграция с жизнеобеспечением
pub struct EvolutionEngine {
    memory: EvolutionaryMemory,
    last_cycle: u64,
    cycle_interval: u64,
}

impl EvolutionEngine {
    pub fn new() -> Self {
        Self {
            memory: EvolutionaryMemory::new(),
            last_cycle: 0,
            cycle_interval: EVOLUTION_CYCLE_SECS,
        }
    }
    
    /// Запуск эволюционного цикла
    pub fn run_cycle(&mut self) -> Vec<EvolutionEvent> {
        let now = self.current_timestamp();
        
        if now - self.last_cycle >= self.cycle_interval {
            self.last_cycle = now;
            return self.memory.evolution_cycle();
        }
        
        Vec::new()
    }
    
    /// Обновление на основе метрик сети
    pub fn update_from_metrics(&mut self, success_rate: f64, network_health: f64) {
        let genes = self.memory.get_all_genes();
        
        for gene in genes {
            let combined_fitness = (success_rate * 0.6 + network_health * 0.4).clamp(0.0, 1.0);
            self.memory.evaluate_fitness(gene.id, combined_fitness);
        }
    }
    
    /// Получение текущей конфигурации протокола (адаптированная)
    pub fn get_adapted_config(&self) -> AdaptedConfig {
        AdaptedConfig {
            poe_weight_decay: self.memory.get_best_gene_value("poe_weight_decay").unwrap_or(0.00001157),
            quarantine_threshold: self.memory.get_best_gene_value("quarantine_threshold").unwrap_or(0.3),
            tension_threshold: self.memory.get_best_gene_value("tension_threshold").unwrap_or(0.7),
            hot_threshold: self.memory.get_best_gene_value("hot_threshold").unwrap_or(0.7),
            cold_threshold: self.memory.get_best_gene_value("cold_threshold").unwrap_or(0.1),
            metabolism_interval_hours: self.memory.get_best_gene_value("metabolism_interval_hours").unwrap_or(1.0),
            rewarm_cost: self.memory.get_best_gene_value("rewarm_cost").unwrap_or(10.0),
            ultrasound_freq_khz: self.memory.get_best_gene_value("ultrasound_freq_khz").unwrap_or(19.0),
            mutation_rate: self.memory.get_best_gene_value("mutation_rate").unwrap_or(0.01),
        }
    }
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Адаптированная конфигурация протокола
#[derive(Debug)]
pub struct AdaptedConfig {
    pub poe_weight_decay: f64,
    pub quarantine_threshold: f64,
    pub tension_threshold: f64,
    pub hot_threshold: f64,
    pub cold_threshold: f64,
    pub metabolism_interval_hours: f64,
    pub rewarm_cost: f64,
    pub ultrasound_freq_khz: f64,
    pub mutation_rate: f64,
}

impl Default for EvolutionaryMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for EvolutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_public_key() -> PublicKey {
        let bytes = [1u8; 32];
        PublicKey::from_bytes(&bytes).unwrap()
    }
    
    #[test]
    fn test_gene_initialization() {
        let evolution = EvolutionaryMemory::new();
        let genes = evolution.get_all_genes();
        
        assert!(!genes.is_empty());
        
        // Проверяем конкретные гены
        assert!(evolution.get_gene_value("poe_weight_decay").is_some());
        assert!(evolution.get_gene_value("quarantine_threshold").is_some());
    }
    
    #[test]
    fn test_gene_mutation() {
        let mut evolution = EvolutionaryMemory::new();
        let gene_id = evolution.get_all_genes()[0].id;
        let original_value = evolution.get_gene_value(&evolution.get_all_genes()[0].name).unwrap();
        
        let mutation = evolution.mutate_gene(gene_id);
        
        // Мутация может не произойти (вероятность 1%)
        if let Some(event) = mutation {
            assert_eq!(event.event_type, EvolutionEventType::Mutation);
            assert_ne!(event.old_value, event.new_value);
        }
    }
    
    #[test]
    fn test_fitness_evaluation() {
        let mut evolution = EvolutionaryMemory::new();
        let gene = evolution.get_all_genes()[0].clone();
        
        let fitness = evolution.evaluate_fitness(gene.id, 0.8);
        assert!(fitness >= 0.0 && fitness <= 1.0);
    }
    
    #[test]
    fn test_crossbreed() {
        let mut evolution = EvolutionaryMemory::new();
        let genes = evolution.get_all_genes();
        
        if genes.len() >= 2 {
            let child_id = evolution.crossbreed(genes[0].id, genes[1].id);
            
            // Скрещивание только для одинаковых имён
            if genes[0].name == genes[1].name {
                assert!(child_id.is_some());
            }
        }
    }
    
    #[test]
    fn test_evolution_cycle() {
        let mut evolution = EvolutionaryMemory::new();
        let events = evolution.evolution_cycle();
        
        // Цикл может не произвести мутаций
        assert!(events.len() <= evolution.get_all_genes().len());
    }
    
    #[test]
    fn test_get_best_gene() {
        let evolution = EvolutionaryMemory::new();
        
        let best = evolution.get_best_gene_value("poe_weight_decay");
        assert!(best.is_some());
    }
    
    #[test]
    fn test_evolution_engine() {
        let mut engine = EvolutionEngine::new();
        
        engine.update_from_metrics(0.75, 0.8);
        let config = engine.get_adapted_config();
        
        assert!(config.poe_weight_decay > 0.0);
        assert!(config.quarantine_threshold >= 0.1 && config.quarantine_threshold <= 0.5);
    }
}

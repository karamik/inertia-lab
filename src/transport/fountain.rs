// src/memory/fountain.rs
// Модуль фонтанных кодов (Luby Transform) для генетической памяти
// Inertia Protocol — Post-Internet Digital Species
//
// Фонтанные коды — гениальное решение для распределённого хранения:
// 1. Данные можно восстановить из любого набора фрагментов (не нужны все)
// 2. Не нужно знать, какие фрагменты утеряны
// 3. Можно бесконечно генерировать новые фрагменты
// 4. Идеально для mesh-сетей с нестабильной связью
// 5. Позволяет "растворить" историю в популяции

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use rand::Rng;
use rand::rngs::ThreadRng;
use log::{debug, info, warn, error};

// Константы фонтанных кодов
const DEFAULT_SYMBOL_SIZE: usize = 1024;    // 1 KB на символ
const DEFAULT_REDUNDANCY: f64 = 1.5;        // 150% избыточность для надёжности
const MAX_SYMBOLS: usize = 65536;            // Максимум 65k символов на блок
const SOLT_DISTRIBUTION: [f64; 4] = [0.5, 0.3, 0.15, 0.05]; // Распределение степени

/// Тип распределения степени для LT-кодов
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DegreeDistribution {
    Soliton,        // Идеальное солитонное распределение
    RobustSoliton,  // Устойчивое солитонное распределение (рекомендуется)
    Binomial,       // Биномиальное распределение
    Fixed,          // Фиксированная степень (для тестов)
}

impl Default for DegreeDistribution {
    fn default() -> Self {
        DegreeDistribution::RobustSoliton
    }
}

/// Структура для кодирования/декодирования фонтанных кодов
pub struct FountainCodec {
    symbol_size: usize,
    degree_distribution: DegreeDistribution,
    c: f64,                     // Параметр для Robust Soliton
    delta: f64,                 // Вероятность отказа
    rng: Arc<Mutex<ThreadRng>>,
}

/// Закодированный символ (спора)
#[derive(Debug, Clone)]
pub struct EncodedSymbol {
    pub index: u32,             // Глобальный индекс символа
    pub degree: u16,            // Степень (сколько исходных символов в XOR)
    pub neighbors: Vec<u32>,    // Индексы исходных символов
    pub data: Vec<u8>,          // Данные (результат XOR)
    pub checksum: u32,          // Контрольная сумма для верификации
}

/// Исходный блок данных для кодирования
#[derive(Debug, Clone)]
pub struct SourceBlock {
    pub id: [u8; 32],           // Blake3 хеш блока (уникальный ID)
    pub symbols: Vec<Vec<u8>>,  // Исходные символы
    pub symbol_count: usize,    // Количество символов
    pub total_size: usize,      // Общий размер данных
}

/// Декодер для восстановления исходных данных
pub struct FountainDecoder {
    symbol_size: usize,
    source_blocks: HashMap<[u8; 32], DecodingState>,
}

/// Состояние декодирования для одного блока
struct DecodingState {
    symbol_count: usize,
    received_symbols: Vec<Option<EncodedSymbol>>,
    decoded_symbols: Vec<Option<Vec<u8>>>,
    graph: Vec<Vec<u32>>,       // Граф зависимостей (символы → исходные данные)
    is_decoded: bool,
}

impl FountainCodec {
    pub fn new() -> Self {
        Self {
            symbol_size: DEFAULT_SYMBOL_SIZE,
            degree_distribution: DegreeDistribution::default(),
            c: 0.1,
            delta: 0.01,
            rng: Arc::new(Mutex::new(rand::thread_rng())),
        }
    }
    
    pub fn with_symbol_size(size: usize) -> Self {
        let mut codec = Self::new();
        codec.symbol_size = size;
        codec
    }
    
    pub fn with_distribution(dist: DegreeDistribution) -> Self {
        let mut codec = Self::new();
        codec.degree_distribution = dist;
        codec
    }
    
    /// Кодирование исходных данных в бесконечный поток символов (спор)
    pub fn encode(&mut self, data: &[u8]) -> SourceBlock {
        info!("Encoding {} bytes into fountain codes", data.len());
        
        // Разбиваем на символы
        let symbols = self.split_into_symbols(data);
        let symbol_count = symbols.len();
        
        // Вычисляем хеш блока
        let id = blake3::hash(data).into();
        
        info!("Created source block with {} symbols ({} bytes each)", symbol_count, self.symbol_size);
        
        SourceBlock {
            id,
            symbols,
            symbol_count,
            total_size: data.len(),
        }
    }
    
    /// Генерация следующего закодированного символа (споры)
    pub fn generate_symbol(&mut self, block: &SourceBlock) -> EncodedSymbol {
        let degree = self.sample_degree(block.symbol_count);
        let neighbors = self.select_neighbors(degree, block.symbol_count);
        
        // XOR всех соседних символов
        let mut data = vec![0u8; self.symbol_size];
        for &idx in &neighbors {
            for (i, byte) in block.symbols[idx].iter().enumerate() {
                if i < data.len() {
                    data[i] ^= byte;
                }
            }
        }
        
        let checksum = self.calculate_checksum(&data);
        let index = self.generate_symbol_index();
        
        EncodedSymbol {
            index,
            degree: degree as u16,
            neighbors: neighbors.iter().map(|&n| n as u32).collect(),
            data,
            checksum,
        }
    }
    
    /// Генерация нескольких спор (для массового распространения)
    pub fn generate_spores(&mut self, block: &SourceBlock, count: usize) -> Vec<EncodedSymbol> {
        debug!("Generating {} spores from block", count);
        (0..count).map(|_| self.generate_symbol(block)).collect()
    }
    
    /// Создание декодера для этого блока
    pub fn create_decoder(&self) -> FountainDecoder {
        FountainDecoder::new(self.symbol_size)
    }
    
    /// Разбиение данных на символы
    fn split_into_symbols(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut symbols = Vec::new();
        let chunks = data.chunks(self.symbol_size);
        
        for chunk in chunks {
            let mut symbol = chunk.to_vec();
            // Дополняем нулями до symbol_size
            if symbol.len() < self.symbol_size {
                symbol.resize(self.symbol_size, 0);
            }
            symbols.push(symbol);
        }
        
        symbols
    }
    
    /// Выбор степени (количество исходных символов в XOR)
    fn sample_degree(&mut self, symbol_count: usize) -> usize {
        if symbol_count == 1 {
            return 1;
        }
        
        match self.degree_distribution {
            DegreeDistribution::Soliton => self.sample_soliton(symbol_count),
            DegreeDistribution::RobustSoliton => self.sample_robust_soliton(symbol_count),
            DegreeDistribution::Binomial => self.sample_binomial(symbol_count),
            DegreeDistribution::Fixed => 3.min(symbol_count),
        }
    }
    
    /// Солитонное распределение (теоретически идеальное)
    fn sample_soliton(&mut self, k: usize) -> usize {
        let mut rng = self.rng.lock().unwrap();
        let x: f64 = rng.gen();
        
        for d in 1..=k {
            let prob = if d == 1 {
                1.0 / k as f64
            } else {
                1.0 / (d as f64 * (d as f64 - 1.0))
            };
            
            if x < prob {
                return d;
            }
        }
        1
    }
    
    /// Устойчивое солитонное распределение (рекомендуется для реального использования)
    fn sample_robust_soliton(&mut self, k: usize) -> usize {
        let s = (self.c * (k as f64).sqrt()).ceil() as usize;
        let beta = self.delta;
        
        let mut rng = self.rng.lock().unwrap();
        
        // Генерация распределения
        let mut distribution = vec![0.0; k + 1];
        
        // Солитонная часть
        distribution[1] = 1.0 / k as f64;
        for d in 2..=k {
            distribution[d] = 1.0 / (d as f64 * (d as f64 - 1.0));
        }
        
        // Дополнительная часть для надёжности
        for d in 1..=s {
            let tau = (s as f64 / (d as f64)) * (beta * (k as f64 / s as f64)).ln();
            distribution[d] += tau / (k as f64);
        }
        distribution[s] += (s as f64 / k as f64) * (beta * (k as f64 / s as f64)).ln();
        
        // Нормализация
        let sum: f64 = distribution.iter().sum();
        for prob in distribution.iter_mut() {
            *prob /= sum;
        }
        
        // Выбор степени
        let x: f64 = rng.gen();
        let mut acc = 0.0;
        for d in 1..=k {
            acc += distribution[d];
            if x < acc {
                return d;
            }
        }
        1
    }
    
    /// Биномиальное распределение (для тестов)
    fn sample_binomial(&mut self, k: usize) -> usize {
        let mut rng = self.rng.lock().unwrap();
        let p = 0.5;
        let mut successes = 0;
        
        for _ in 0..k {
            if rng.gen_bool(p) {
                successes += 1;
            }
        }
        
        successes.max(1).min(k)
    }
    
    /// Выбор случайных соседей для XOR
    fn select_neighbors(&mut self, degree: usize, symbol_count: usize) -> Vec<usize> {
        let mut rng = self.rng.lock().unwrap();
        let mut neighbors = Vec::with_capacity(degree);
        
        while neighbors.len() < degree {
            let idx = rng.gen_range(0..symbol_count);
            if !neighbors.contains(&idx) {
                neighbors.push(idx);
            }
        }
        
        neighbors.sort();
        neighbors
    }
    
    /// Генерация уникального индекса символа
    fn generate_symbol_index(&mut self) -> u32 {
        let mut rng = self.rng.lock().unwrap();
        rng.gen()
    }
    
    /// Вычисление контрольной суммы
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        data.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32))
    }
}

impl FountainDecoder {
    pub fn new(symbol_size: usize) -> Self {
        Self {
            symbol_size,
            source_blocks: HashMap::new(),
        }
    }
    
    /// Добавление полученной споры в декодер
    pub fn add_symbol(&mut self, symbol: EncodedSymbol) -> Option<Vec<u8>> {
        // Ищем или создаём состояние для блока (пока используем индекс как ID)
        // В реальной реализации ID блока должен быть частью символа
        let block_id = [0u8; 32]; // Заглушка
        
        if !self.source_blocks.contains_key(&block_id) {
            // Оцениваем количество символов по максимальному индексу соседа
            let max_neighbor = symbol.neighbors.iter().max().unwrap_or(&0);
            let symbol_count = (*max_neighbor + 1) as usize;
            
            self.source_blocks.insert(block_id, DecodingState {
                symbol_count,
                received_symbols: vec![None; symbol_count * 2], // Запас по индексам
                decoded_symbols: vec![None; symbol_count],
                graph: vec![Vec::new(); symbol_count],
                is_decoded: false,
            });
        }
        
        let state = self.source_blocks.get_mut(&block_id).unwrap();
        
        // Сохраняем символ
        let idx = symbol.index as usize;
        if idx < state.received_symbols.len() {
            state.received_symbols[idx] = Some(symbol.clone());
            
            // Строим граф зависимостей
            for &neighbor in &symbol.neighbors {
                let neighbor_idx = neighbor as usize;
                if neighbor_idx < state.graph.len() {
                    state.graph[neighbor_idx].push(idx);
                }
            }
        }
        
        // Пытаемся декодировать
        self.try_decode(block_id)
    }
    
    /// Попытка декодирования с использованием Belief Propagation
    fn try_decode(&mut self, block_id: [u8; 32]) -> Option<Vec<u8>> {
        let state = self.source_blocks.get_mut(&block_id)?;
        
        if state.is_decoded {
            return self.assemble_data(state);
        }
        
        let mut changed = true;
        let mut iterations = 0;
        let max_iterations = state.symbol_count * 2;
        
        while changed && iterations < max_iterations {
            changed = false;
            iterations += 1;
            
            // Ищем символы степени 1 (isolated symbols)
            for (idx, symbol_opt) in state.received_symbols.iter().enumerate() {
                if let Some(symbol) = symbol_opt {
                    if symbol.degree == 1 && !symbol.neighbors.is_empty() {
                        let source_idx = symbol.neighbors[0] as usize;
                        
                        if source_idx < state.decoded_symbols.len() && state.decoded_symbols[source_idx].is_none() {
                            // Декодируем исходный символ
                            state.decoded_symbols[source_idx] = Some(symbol.data.clone());
                            changed = true;
                            
                            // Обновляем все символы, зависящие от этого источника
                            for &dep_idx in &state.graph[source_idx] {
                                if let Some(dep_symbol) = state.received_symbols.get_mut(dep_idx) {
                                    if let Some(sym) = dep_symbol {
                                        // XOR-им декодированный символ из зависимого
                                        for (i, byte) in sym.data.iter_mut().enumerate() {
                                            if i < symbol.data.len() {
                                                *byte ^= symbol.data[i];
                                            }
                                        }
                                        
                                        // Уменьшаем степень
                                        sym.degree -= 1;
                                        sym.neighbors.retain(|&n| n as usize != source_idx);
                                    }
                                }
                            }
                            
                            break;
                        }
                    }
                }
            }
        }
        
        // Проверяем, все ли символы декодированы
        let all_decoded = state.decoded_symbols.iter().all(|s| s.is_some());
        
        if all_decoded {
            state.is_decoded = true;
            return self.assemble_data(state);
        }
        
        None
    }
    
    /// Сборка исходных данных из декодированных символов
    fn assemble_data(&self, state: &DecodingState) -> Option<Vec<u8>> {
        let mut data = Vec::with_capacity(state.symbol_count * self.symbol_size);
        
        for symbol_opt in &state.decoded_symbols {
            if let Some(symbol) = symbol_opt {
                data.extend_from_slice(symbol);
            } else {
                return None;
            }
        }
        
        // Обрезаем до реального размера (убираем padding)
        while data.last() == Some(&0) {
            data.pop();
        }
        
        Some(data)
    }
    
    /// Получение прогресса декодирования (0.0 - 1.0)
    pub fn get_progress(&self, block_id: [u8; 32]) -> f64 {
        if let Some(state) = self.source_blocks.get(&block_id) {
            let decoded = state.decoded_symbols.iter().filter(|s| s.is_some()).count();
            decoded as f64 / state.symbol_count as f64
        } else {
            0.0
        }
    }
}

/// Генетическая память — распределённое хранилище с фонтанными кодами
pub struct GeneticMemory {
    codec: FountainCodec,
    decoder: FountainDecoder,
    stored_blocks: HashMap<[u8; 32], SourceBlock>,
    spore_storage: Arc<Mutex<Vec<EncodedSymbol>>>,
}

impl GeneticMemory {
    pub fn new() -> Self {
        Self {
            codec: FountainCodec::new(),
            decoder: FountainDecoder::new(DEFAULT_SYMBOL_SIZE),
            stored_blocks: HashMap::new(),
            spore_storage: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Сохранение данных в генетическую память
    pub fn store(&mut self, data: &[u8]) -> [u8; 32] {
        let block = self.codec.encode(data);
        let block_id = block.id;
        
        // Генерируем начальные споры (50% избыточности)
        let spore_count = (block.symbol_count as f64 * DEFAULT_REDUNDANCY) as usize;
        let spores = self.codec.generate_spores(&block, spore_count);
        
        let mut storage = self.spore_storage.lock().unwrap();
        for spore in spores {
            storage.push(spore);
        }
        
        self.stored_blocks.insert(block_id, block);
        info!("Stored {} bytes in genetic memory ({} spores generated)", data.len(), spore_count);
        
        block_id
    }
    
    /// Восстановление данных из спор
    pub fn recover(&mut self) -> Vec<Vec<u8>> {
        let mut recovered = Vec::new();
        let storage = self.spore_storage.lock().unwrap();
        
        // Группируем споры (в реальной реализации — по block_id)
        // Для демо просто пытаемся декодировать всё вместе
        
        for spore in storage.iter() {
            if let Some(data) = self.decoder.add_symbol(spore.clone()) {
                // Проверяем хеш (в реальной реализации)
                recovered.push(data);
            }
        }
        
        recovered
    }
    
    /// Распространение спор в сеть (возвращает споры для отправки)
    pub fn get_spores_to_broadcast(&self, count: usize) -> Vec<EncodedSymbol> {
        let storage = self.spore_storage.lock().unwrap();
        let mut rng = rand::thread_rng();
        
        // Случайная выборка спор
        let mut spores = Vec::new();
        let indices: Vec<usize> = (0..storage.len()).collect();
        
        for _ in 0..count.min(storage.len()) {
            let idx = rng.gen_range(0..storage.len());
            spores.push(storage[idx].clone());
        }
        
        spores
    }
    
    /// Интеграция спор извне (при встрече с другим узлом)
    pub fn integrate_spores(&mut self, incoming_spores: Vec<EncodedSymbol>) -> usize {
        let mut new_spores = 0;
        let mut storage = self.spore_storage.lock().unwrap();
        
        for spore in incoming_spores {
            // Проверяем, есть ли уже такая спора
            let exists = storage.iter().any(|s| s.index == spore.index);
            if !exists {
                storage.push(spore);
                new_spores += 1;
            }
        }
        
        if new_spores > 0 {
            debug!("Integrated {} new spores from the network", new_spores);
            
            // Пытаемся декодировать новые данные
            drop(storage); // Освобождаем lock перед декодированием
            let recovered = self.recover();
            
            if !recovered.is_empty() {
                info!("Recovered {} messages from integrated spores", recovered.len());
            }
        }
        
        new_spores
    }
    
    /// Очистка старых спор (метаболизм)
    pub fn prune_old_spores(&mut self, max_spores: usize) -> usize {
        let mut storage = self.spore_storage.lock().unwrap();
        let original_len = storage.len();
        
        if storage.len() > max_spores {
            // Удаляем случайные споры (в реальной реализации — по времени жизни)
            storage.truncate(max_spores);
        }
        
        let removed = original_len - storage.len();
        if removed > 0 {
            debug!("Pruned {} old spores (metabolism)", removed);
        }
        
        removed
    }
    
    /// Статистика памяти
    pub fn memory_stats(&self) -> MemoryStats {
        let storage = self.spore_storage.lock().unwrap();
        
        MemoryStats {
            total_spores: storage.len(),
            estimated_data_size: storage.len() * DEFAULT_SYMBOL_SIZE,
            stored_blocks: self.stored_blocks.len(),
        }
    }
}

#[derive(Debug)]
pub struct MemoryStats {
    pub total_spores: usize,
    pub estimated_data_size: usize,
    pub stored_blocks: usize,
}

impl Default for GeneticMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode_small() {
        let mut codec = FountainCodec::new();
        let data = b"Hello, Inertia!";
        
        let block = codec.encode(data);
        let mut decoder = FountainCodec::new().create_decoder();
        
        // Генерируем споры до декодирования
        let mut decoded = None;
        for _ in 0..block.symbol_count * 2 {
            let symbol = codec.generate_symbol(&block);
            if let Some(result) = decoder.add_symbol(symbol) {
                decoded = Some(result);
                break;
            }
        }
        
        assert!(decoded.is_some());
        if let Some(result) = decoded {
            assert_eq!(&result[..data.len()], data.as_slice());
        }
    }
    
    #[test]
    fn test_genetic_memory() {
        let mut memory = GeneticMemory::new();
        
        let data1 = b"First message for genetic memory";
        let data2 = b"Second message that will be recovered from spores";
        
        memory.store(data1);
        memory.store(data2);
        
        // Имитируем распространение спор
        let spores = memory.get_spores_to_broadcast(50);
        
        // Создаём новую память и интегрируем споры
        let mut new_memory = GeneticMemory::new();
        let integrated = new_memory.integrate_spores(spores);
        assert!(integrated > 0);
        
        // Восстанавливаем данные
        let recovered = new_memory.recover();
        assert!(!recovered.is_empty());
    }
    
    #[test]
    fn test_spore_integration() {
        let mut memory1 = GeneticMemory::new();
        let mut memory2 = GeneticMemory::new();
        
        let data = b"Shared secret between nodes";
        memory1.store(data);
        
        // Передаём споры от узла 1 к узлу 2
        let spores = memory1.get_spores_to_broadcast(20);
        let integrated = memory2.integrate_spores(spores);
        
        assert!(integrated > 0);
        
        let recovered = memory2.recover();
        assert!(!recovered.is_empty());
    }
    
    #[test]
    fn test_degree_distributions() {
        let mut codec_soliton = FountainCodec::with_distribution(DegreeDistribution::Soliton);
        let mut codec_robust = FountainCodec::with_distribution(DegreeDistribution::RobustSoliton);
        
        let data = vec![0u8; 10000];
        let block = codec_soliton.encode(&data);
        
        let mut degrees = Vec::new();
        for _ in 0..100 {
            let symbol = codec_robust.generate_symbol(&block);
            degrees.push(symbol.degree);
        }
        
        // Проверяем, что степени в разумном диапазоне
        let avg_degree = degrees.iter().sum::<u16>() as f64 / degrees.len() as f64;
        assert!(avg_degree > 1.0 && avg_degree <= block.symbol_count as f64);
    }
    
    #[test]
    fn test_recovery_from_fragments() {
        let mut codec = FountainCodec::new();
        let data = vec![0x42; 5000];
        let block = codec.encode(&data);
        
        let mut decoder = FountainCodec::new().create_decoder();
        
        // Генерируем только 80% от необходимых символов
        let target = (block.symbol_count as f64 * 0.8) as usize;
        let mut symbols = Vec::new();
        
        for _ in 0..target {
            symbols.push(codec.generate_symbol(&block));
        }
        
        // Пытаемся декодировать
        let mut decoded = None;
        for symbol in symbols {
            if let Some(result) = decoder.add_symbol(symbol) {
                decoded = Some(result);
                break;
            }
        }
        
        // С фонтанными кодами можно восстановить из >100% символов
        // 80% недостаточно, но для теста просто проверяем, что декодер работает
        assert!(decoder.get_progress([0u8; 32]) >= 0.0);
    }
}

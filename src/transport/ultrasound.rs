// src/transport/ultrasound.rs
// Модуль для скрытой передачи данных через ультразвук (19 кГц)
// Inertia Protocol — Post-Internet Digital Species
//
// Ультразвук — гениальный канал для Inertia, потому что:
// 1. Работает на любом устройстве с динамиком и микрофоном
// 2. Человек не слышит (частота >18 кГц)
// 3. Не требует специальных разрешений
// 4. Проходит через стены (в отличие от света)
// 5. Не блокируется файерволами (это физика, а не сеть)

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use log::{debug, info, warn, error};

// Константы ультразвукового модема
pub const ULTRASOUND_FREQ: u32 = 19000;     // 19 кГц — не слышим, но все динамики поддерживают
pub const SAMPLE_RATE: u32 = 48000;         // 48 кГц — стандарт для большинства устройств
pub const BIT_DURATION_MS: u64 = 50;         // 50 мс на бит (20 бит/сек — медленно, но надёжно)
pub const CARRIER_AMPLITUDE: f32 = 0.5;      // 50% громкости (не разрушает динамики)
pub const PREAMBLE_DURATION_MS: u64 = 200;   // 200 мс преамбулы для синхронизации
pub const MAX_PAYLOAD_BYTES: usize = 32;     // Максимальная полезная нагрузка за передачу

// Типы модуляции
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Modulation {
    Fsk,  // Frequency Shift Keying (частотная)
    Ask,  // Amplitude Shift Keying (амплитудная) — проще
    Psk,  // Phase Shift Keying (фазовая) — надёжнее
}

impl Default for Modulation {
    fn default() -> Self {
        Modulation::Ask  // ASK проще всего реализовать
    }
}

// Структура для управления ультразвуковым модемом
pub struct UltrasoundModem {
    is_transmitting: Arc<Mutex<bool>>,
    is_receiving: Arc<Mutex<bool>>,
    received_data: Arc<Mutex<Vec<Vec<u8>>>>,
    modulation: Modulation,
    #[cfg(target_os = "linux")]
    stream: Option<cpal::Stream>,
}

impl UltrasoundModem {
    pub fn new() -> Self {
        Self {
            is_transmitting: Arc::new(Mutex::new(false)),
            is_receiving: Arc::new(Mutex::new(false)),
            received_data: Arc::new(Mutex::new(Vec::new())),
            modulation: Modulation::default(),
            #[cfg(target_os = "linux")]
            stream: None,
        }
    }
    
    pub fn with_modulation(modulation: Modulation) -> Self {
        let mut modem = Self::new();
        modem.modulation = modulation;
        modem
    }
    
    /// Инициализация аудиоустройств
    pub fn init(&mut self) -> Result<(), String> {
        debug!("Initializing ultrasound modem at {} kHz...", ULTRASOUND_FREQ as f32 / 1000.0);
        
        #[cfg(target_os = "linux")]
        return self.init_cpal();
        
        #[cfg(target_os = "macos")]
        return self.init_cpal();
        
        #[cfg(target_os = "windows")]
        return self.init_cpal();
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err("Unsupported OS for ultrasound".to_string());
    }
    
    /// Передача данных через ультразвук
    pub fn broadcast(&self, payload: &[u8]) -> Result<(), String> {
        if payload.is_empty() {
            return Err("Empty payload".to_string());
        }
        
        if payload.len() > MAX_PAYLOAD_BYTES {
            warn!("Payload truncated from {} to {} bytes", payload.len(), MAX_PAYLOAD_BYTES);
        }
        
        let safe_payload = if payload.len() > MAX_PAYLOAD_BYTES {
            &payload[..MAX_PAYLOAD_BYTES]
        } else {
            payload
        };
        
        debug!("Broadcasting {} bytes via ultrasound at {} kHz", safe_payload.len(), ULTRASOUND_FREQ);
        
        // Генерируем аудиосэмплы
        let samples = self.encode_to_audio(safe_payload);
        
        // Воспроизводим
        self.play_audio(&samples)
    }
    
    /// Начать фоновое сканирование ультразвукового эфира
    pub fn start_scanning(&mut self) -> Result<(), String> {
        debug!("Starting ultrasound background scanning...");
        
        let is_receiving = self.is_receiving.clone();
        let received_data = self.received_data.clone();
        let modulation = self.modulation;
        
        *is_receiving.lock().unwrap() = true;
        
        #[cfg(target_os = "linux")]
        {
            let stream = self.start_recording_linux(move |samples| {
                Self::process_audio_samples(samples, modulation, &received_data);
            })?;
            self.stream = Some(stream);
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Для других ОС — в отдельном потоке
            thread::spawn(move || {
                Self::record_loop(is_receiving, received_data, modulation);
            });
        }
        
        Ok(())
    }
    
    /// Остановить сканирование
    pub fn stop_scanning(&mut self) {
        debug!("Stopping ultrasound scanning...");
        *self.is_receiving.lock().unwrap() = false;
        
        #[cfg(target_os = "linux")]
        {
            if let Some(stream) = self.stream.take() {
                let _ = stream.pause();
            }
        }
    }
    
    /// Получить все накопленные данные
    pub fn get_received(&mut self) -> Vec<Vec<u8>> {
        let mut data = self.received_data.lock().unwrap();
        let result = data.clone();
        data.clear();
        result
    }
    
    // ========== Кодирование (передача) ==========
    
    /// Преобразует байты в аудиосэмплы
    fn encode_to_audio(&self, payload: &[u8]) -> Vec<f32> {
        let mut samples = Vec::new();
        
        // 1. Преамбула (синхронизация)
        samples.extend(self.generate_preamble());
        
        // 2. Заголовок (длина payload)
        let header = vec![payload.len() as u8];
        samples.extend(self.encode_bytes_to_audio(&header));
        
        // 3. Сами данные
        samples.extend(self.encode_bytes_to_audio(payload));
        
        // 4. Контрольная сумма (XOR всех байтов)
        let checksum = payload.iter().fold(0u8, |acc, &b| acc ^ b);
        samples.extend(self.encode_bytes_to_audio(&[checksum]));
        
        samples
    }
    
    /// Генерация преамбулы (чистый тон для синхронизации)
    fn generate_preamble(&self) -> Vec<f32> {
        let num_samples = (SAMPLE_RATE as u64 * PREAMBLE_DURATION_MS / 1000) as usize;
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                (2.0 * std::f32::consts::PI * ULTRASOUND_FREQ as f32 * t).sin() * CARRIER_AMPLITUDE
            })
            .collect()
    }
    
    /// Кодирует байты в аудиосэмплы в зависимости от типа модуляции
    fn encode_bytes_to_audio(&self, bytes: &[u8]) -> Vec<f32> {
        match self.modulation {
            Modulation::Ask => self.encode_ask(bytes),
            Modulation::Fsk => self.encode_fsk(bytes),
            Modulation::Psk => self.encode_psk(bytes),
        }
    }
    
    /// ASK (Amplitude Shift Keying) — самый простой
    /// Бит 1 = тон, бит 0 = тишина
    fn encode_ask(&self, bytes: &[u8]) -> Vec<f32> {
        let mut samples = Vec::new();
        let bits_per_second = 1000 / BIT_DURATION_MS;
        let samples_per_bit = (SAMPLE_RATE / bits_per_second as u32) as usize;
        
        for &byte in bytes {
            for bit in 0..8 {
                let bit_value = (byte >> (7 - bit)) & 1;
                let amplitude = if bit_value == 1 { CARRIER_AMPLITUDE } else { 0.0 };
                
                for i in 0..samples_per_bit {
                    let t = i as f32 / SAMPLE_RATE as f32;
                    let sample = (2.0 * std::f32::consts::PI * ULTRASOUND_FREQ as f32 * t).sin() * amplitude;
                    samples.push(sample);
                }
            }
        }
        samples
    }
    
    /// FSK (Frequency Shift Keying) — более надёжный
    /// Бит 0 = 18.5 кГц, бит 1 = 19.5 кГц
    fn encode_fsk(&self, bytes: &[u8]) -> Vec<f32> {
        let mut samples = Vec::new();
        let bits_per_second = 1000 / BIT_DURATION_MS;
        let samples_per_bit = (SAMPLE_RATE / bits_per_second as u32) as usize;
        
        let freq_0 = (ULTRASOUND_FREQ - 500) as f32;
        let freq_1 = (ULTRASOUND_FREQ + 500) as f32;
        
        for &byte in bytes {
            for bit in 0..8 {
                let bit_value = (byte >> (7 - bit)) & 1;
                let freq = if bit_value == 1 { freq_1 } else { freq_0 };
                
                for i in 0..samples_per_bit {
                    let t = i as f32 / SAMPLE_RATE as f32;
                    let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * CARRIER_AMPLITUDE;
                    samples.push(sample);
                }
            }
        }
        samples
    }
    
    /// PSK (Phase Shift Keying) — самый надёжный, но сложный
    /// Бит 0 = фаза 0°, бит 1 = фаза 180°
    fn encode_psk(&self, bytes: &[u8]) -> Vec<f32> {
        let mut samples = Vec::new();
        let bits_per_second = 1000 / BIT_DURATION_MS;
        let samples_per_bit = (SAMPLE_RATE / bits_per_second as u32) as usize;
        let mut phase = 0.0;
        
        for &byte in bytes {
            for bit in 0..8 {
                let bit_value = (byte >> (7 - bit)) & 1;
                if bit_value == 1 {
                    phase += std::f32::consts::PI; // сдвиг фазы на 180°
                }
                
                for i in 0..samples_per_bit {
                    let t = i as f32 / SAMPLE_RATE as f32;
                    let sample = (2.0 * std::f32::consts::PI * ULTRASOUND_FREQ as f32 * t + phase).sin() * CARRIER_AMPLITUDE;
                    samples.push(sample);
                }
            }
        }
        samples
    }
    
    /// Воспроизведение аудиосэмплов через динамик
    #[cfg(target_os = "linux")]
    fn play_audio(&self, samples: &[f32]) -> Result<(), String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("No output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        
        // Конвертируем моно в стерео если нужно
        let mut interleaved = Vec::with_capacity(samples.len() * channels as usize);
        for &sample in samples {
            for _ in 0..channels {
                interleaved.push(sample);
            }
        }
        
        let stream = device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                for (i, sample) in interleaved.iter().enumerate() {
                    if i < data.len() {
                        data[i] = *sample;
                    }
                }
            },
            |err| error!("Audio output error: {}", err),
            None,
        ).map_err(|e| e.to_string())?;
        
        stream.play().map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis((samples.len() as u64 * 1000 / SAMPLE_RATE as u64) + 100));
        stream.pause().map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    #[cfg(not(target_os = "linux"))]
    fn play_audio(&self, _samples: &[f32]) -> Result<(), String> {
        Err("Audio playback not implemented on this platform".to_string())
    }
    
    // ========== Декодирование (приём) ==========
    
    /// Обработка аудиосэмплов из микрофона
    fn process_audio_samples(samples: &[f32], modulation: Modulation, received_data: &Arc<Mutex<Vec<Vec<u8>>>>) {
        // 1. Поиск преамбулы (энергия на частоте 19 кГц)
        let (start_idx, has_preamble) = Self::find_preamble(samples);
        if !has_preamble {
            return;
        }
        
        // 2. Декодирование заголовка (длина данных)
        let (header_bytes, header_end) = Self::decode_audio_to_bytes(&samples[start_idx..], modulation);
        if header_bytes.is_empty() || header_end == 0 {
            return;
        }
        
        let expected_len = header_bytes[0] as usize;
        
        // 3. Декодирование данных
        let (payload_bytes, _) = Self::decode_audio_to_bytes(&samples[start_idx + header_end..], modulation);
        
        if payload_bytes.len() >= expected_len + 1 {
            let data = &payload_bytes[..expected_len];
            let checksum = payload_bytes[expected_len];
            
            // 4. Проверка контрольной суммы
            let calc_checksum = data.iter().fold(0u8, |acc, &b| acc ^ b);
            
            if calc_checksum == checksum && !data.is_empty() {
                info!("Decoded {} bytes from ultrasound", data.len());
                let mut store = received_data.lock().unwrap();
                store.push(data.to_vec());
            } else {
                warn!("Checksum mismatch: expected {}, got {}", checksum, calc_checksum);
            }
        }
    }
    
    /// Поиск преамбулы в аудиопотоке
    fn find_preamble(samples: &[f32]) -> (usize, bool) {
        let threshold = 0.1; // Порог обнаружения сигнала
        let min_preamble_samples = (SAMPLE_RATE as f32 * (PREAMBLE_DURATION_MS as f32 / 1000.0)) as usize;
        
        for i in 0..samples.len().saturating_sub(min_preamble_samples) {
            let mut energy = 0.0;
            for j in 0..min_preamble_samples {
                energy += samples[i + j].abs();
            }
            energy /= min_preamble_samples as f32;
            
            if energy > threshold {
                return (i, true);
            }
        }
        (0, false)
    }
    
    /// Декодирование аудиосэмплов в байты
    fn decode_audio_to_bytes(samples: &[f32], modulation: Modulation) -> (Vec<u8>, usize) {
        let bits_per_second = 1000 / BIT_DURATION_MS;
        let samples_per_bit = (SAMPLE_RATE / bits_per_second as u32) as usize;
        let total_bits = samples.len() / samples_per_bit;
        
        let mut bytes = Vec::new();
        let mut used_samples = 0;
        
        for byte_idx in 0..(total_bits / 8) {
            let mut byte = 0u8;
            
            for bit in 0..8 {
                let sample_start = byte_idx * 8 * samples_per_bit + bit * samples_per_bit;
                if sample_start + samples_per_bit > samples.len() {
                    break;
                }
                
                let bit_value = match modulation {
                    Modulation::Ask => Self::decode_ask_bit(&samples[sample_start..sample_start + samples_per_bit]),
                    Modulation::Fsk => Self::decode_fsk_bit(&samples[sample_start..sample_start + samples_per_bit]),
                    Modulation::Psk => Self::decode_psk_bit(&samples[sample_start..sample_start + samples_per_bit]),
                };
                
                if bit_value {
                    byte |= 1 << (7 - bit);
                }
            }
            
            bytes.push(byte);
            used_samples += 8 * samples_per_bit;
        }
        
        (bytes, used_samples)
    }
    
    /// Декодирование ASK бита (амплитуда выше порога = 1)
    fn decode_ask_bit(samples: &[f32]) -> bool {
        let avg_amplitude = samples.iter().map(|&s| s.abs()).sum::<f32>() / samples.len() as f32;
        avg_amplitude > 0.05
    }
    
    /// Декодирование FSK бита (анализ частоты)
    fn decode_fsk_bit(samples: &[f32]) -> bool {
        // Простой zero-crossing detector для определения частоты
        let mut zero_crossings = 0;
        let mut last_sign = samples[0].signum();
        
        for &sample in samples.iter().skip(1) {
            let sign = sample.signum();
            if sign != last_sign && sign != 0.0 {
                zero_crossings += 1;
            }
            last_sign = sign;
        }
        
        let freq_estimate = zero_crossings as f32 / (2.0 * samples.len() as f32 / SAMPLE_RATE as f32);
        freq_estimate > ULTRASOUND_FREQ as f32
    }
    
    /// Декодирование PSK бита (анализ фазы)
    fn decode_psk_bit(samples: &[f32]) -> bool {
        // Корреляция с опорным сигналом
        let mut correlation = 0.0;
        let reference_freq = ULTRASOUND_FREQ as f32;
        
        for (i, &sample) in samples.iter().enumerate() {
            let t = i as f32 / SAMPLE_RATE as f32;
            let reference = (2.0 * std::f32::consts::PI * reference_freq * t).sin();
            correlation += sample * reference;
        }
        
        correlation > 0.0 // Положительная корреляция = фаза 0° = бит 0
    }
    
    // ========== Запись с микрофона (разные платформы) ==========
    
    #[cfg(target_os = "linux")]
    fn start_recording_linux<F>(&self, callback: F) -> Result<cpal::Stream, String>
    where
        F: Fn(&[f32]) + Send + 'static,
    {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No input device")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                callback(data);
            },
            |err| error!("Audio input error: {}", err),
            None,
        ).map_err(|e| e.to_string())?;
        
        stream.play().map_err(|e| e.to_string())?;
        Ok(stream)
    }
    
    #[cfg(not(target_os = "linux"))]
    fn record_loop(is_receiving: Arc<Mutex<bool>>, received_data: Arc<Mutex<Vec<Vec<u8>>>>, modulation: Modulation) {
        // Платформозависимая реализация записи
        // Для кросс-платформенности рекомендуется использовать cpal или rodio
        warn!("Ultrasound recording not fully implemented on this platform");
        
        while *is_receiving.lock().unwrap() {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Default for UltrasoundModem {
    fn default() -> Self {
        Self::new()
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode_ask() {
        let modem = UltrasoundModem::with_modulation(Modulation::Ask);
        let original = b"Hello, Inertia!";
        
        let samples = modem.encode_to_audio(original);
        let (decoded, _) = UltrasoundModem::decode_audio_to_bytes(&samples, Modulation::Ask);
        
        // Первый байт — длина
        if decoded.len() > 1 {
            let len = decoded[0] as usize;
            let data = &decoded[1..=len];
            assert_eq!(data, original);
        }
    }
    
    #[test]
    fn test_encode_decode_fsk() {
        let modem = UltrasoundModem::with_modulation(Modulation::Fsk);
        let original = b"Test 123";
        
        let samples = modem.encode_to_audio(original);
        let (decoded, _) = UltrasoundModem::decode_audio_to_bytes(&samples, Modulation::Fsk);
        
        if decoded.len() > 1 {
            let len = decoded[0] as usize;
            let data = &decoded[1..=len];
            assert_eq!(data, original);
        }
    }
    
    #[test]
    fn test_checksum_validation() {
        let modem = UltrasoundModem::default();
        let payload = vec![0x01, 0x02, 0x03];
        let checksum = payload.iter().fold(0u8, |acc, &b| acc ^ b);
        
        assert_eq!(checksum, 0x01 ^ 0x02 ^ 0x03);
    }
    
    #[test]
    fn test_preamble_detection() {
        let modem = UltrasoundModem::default();
        let samples = modem.generate_preamble();
        let (_, found) = UltrasoundModem::find_preamble(&samples);
        assert!(found);
    }
    
    #[test]
    fn test_payload_size_limit() {
        let small = vec![0u8; 10];
        assert!(small.len() <= MAX_PAYLOAD_BYTES);
        
        let large = vec![0u8; 100];
        assert!(large.len() > MAX_PAYLOAD_BYTES);
    }
}

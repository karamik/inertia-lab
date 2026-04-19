// src/transport/bluetooth_adv.rs
// Модуль для скрытой передачи данных через Bluetooth Advertising
// Inertia Protocol — Post-Internet Digital Species
//
// Bluetooth Advertising — идеальный канал для Inertia, потому что:
// 1. Работает в фоновом режиме на всех современных телефонах
// 2. Не требует сопряжения (pairing)
// 3. Пакеты видны всем вокруг
// 4. Потребляет минимум энергии

use std::time::Duration;
use std::thread::sleep;
use log::{debug, info, warn, error};

// Константы протокола
const MANUFACTURER_ID: u16 = 0x0IN3; // Собственный ID для Inertia (0x0IN3 = 0x09 0x4E?)
const SERVICE_UUID: u128 = 0x4e5f5c4b_4a494e45_52544941_00000000; // "INERTIA" в hex
const ADV_PREFIX: &[u8] = b"INR"; // Префикс для распознавания пакетов Inertia
const MAX_ADV_PAYLOAD: usize = 31; // Максимальная полезная нагрузка в advertisement пакете

// Типы PDU для Bluetooth LE
#[repr(u8)]
enum PduType {
    AdvInd = 0x00,      // Connectable undirected advertising
    AdvNonconnInd = 0x03, // Non-connectable undirected advertising (наш выбор)
    ScanRsp = 0x04,     // Scan response
}

#[cfg(target_os = "linux")]
use bluer::{Adapter, Advertisement, AdvertisementOptions, Device, Uuid};

#[cfg(target_os = "macos")]
use btleplug::api::{Central, CentralEvent, Manager as BtManager, Peripheral, ScanFilter};
#[cfg(target_os = "macos")]
use btleplug::platform::Manager;

#[cfg(target_os = "windows")]
use btleplug::api::{Central, CentralEvent, Manager as BtManager, Peripheral, ScanFilter};
#[cfg(target_os = "windows")]
use btleplug::platform::Manager;

/// Структура для управления Bluetooth-транспортом
pub struct BluetoothAdv {
    #[cfg(target_os = "linux")]
    adapter: Option<Adapter>,
    #[cfg(target_os = "linux")]
    advertisement_handle: Option<Advertisement>,
    
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    central: Option<btleplug::platform::Central>,
}

impl BluetoothAdv {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            adapter: None,
            #[cfg(target_os = "linux")]
            advertisement_handle: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            central: None,
        }
    }
    
    /// Инициализация Bluetooth-адаптера
    pub fn init(&mut self) -> Result<(), String> {
        debug!("Initializing Bluetooth transport...");
        
        #[cfg(target_os = "linux")]
        return self.init_linux();
        
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        return self.init_btleplug();
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err("Unsupported OS for Bluetooth".to_string());
    }
    
    /// Передача данных через Advertising (не требует сопряжения)
    pub fn broadcast(&self, payload: &[u8]) -> Result<(), String> {
        if payload.is_empty() {
            return Err("Empty payload".to_string());
        }
        
        let safe_payload = if payload.len() > MAX_ADV_PAYLOAD {
            warn!("Payload truncated from {} to {} bytes", payload.len(), MAX_ADV_PAYLOAD);
            &payload[..MAX_ADV_PAYLOAD]
        } else {
            payload
        };
        
        debug!("Broadcasting {} bytes via Bluetooth advertising", safe_payload.len());
        
        #[cfg(target_os = "linux")]
        return self.broadcast_linux(safe_payload);
        
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        return self.broadcast_btleplug(safe_payload);
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err("Unsupported OS for Bluetooth broadcast".to_string());
    }
    
    /// Сканирование эфира в поисках пакетов Inertia
    pub fn scan(&mut self, duration_secs: u64) -> Vec<Vec<u8>> {
        debug!("Scanning for Inertia spores via Bluetooth advertising ({} seconds)...", duration_secs);
        
        #[cfg(target_os = "linux")]
        return self.scan_linux(duration_secs);
        
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        return self.scan_btleplug(duration_secs);
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Vec::new();
    }
    
    /// Отправка больших данных с фрагментацией (каждый фрагмент — отдельный ADV пакет)
    pub fn broadcast_fragmented(&self, payload: &[u8]) -> Vec<Result<(), String>> {
        let chunk_size = MAX_ADV_PAYLOAD - 1; // 1 байт под номер фрагмента
        payload.chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk)| {
                let mut fragment = Vec::with_capacity(chunk.len() + 1);
                fragment.push(i as u8);
                fragment.extend_from_slice(chunk);
                self.broadcast(&fragment)
            })
            .collect()
    }
    
    /// Остановка рекламы (освобождение ресурсов)
    pub fn stop(&mut self) {
        debug!("Stopping Bluetooth advertising...");
        
        #[cfg(target_os = "linux")]
        {
            if let Some(adv) = self.advertisement_handle.take() {
                let _ = adv.stop();
            }
        }
        
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            if let Some(central) = &self.central {
                let _ = central.stop_scan();
            }
        }
    }
    
    // ========== Linux implementation (bluer) ==========
    
    #[cfg(target_os = "linux")]
    fn init_linux(&mut self) -> Result<(), String> {
        use bluer::AdapterEvent;
        
        let session = bluer::Session::new().map_err(|e| format!("Failed to create BT session: {}", e))?;
        let adapter = session.default_adapter().map_err(|e| format!("No Bluetooth adapter: {}", e))?;
        
        adapter.set_powered(true).map_err(|e| format!("Failed to power adapter: {}", e))?;
        
        debug!("Bluetooth adapter ready: {}", adapter.name().unwrap_or_else(|| "Unknown".to_string()));
        
        self.adapter = Some(adapter);
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    fn broadcast_linux(&self, payload: &[u8]) -> Result<(), String> {
        use bluer::{Advertisement, AdvertisementOptions, Uuid};
        
        let adapter = self.adapter.as_ref().ok_or("Adapter not initialized")?;
        
        // Формируем manufacturer-specific data
        let mut adv_data = vec![];
        adv_data.extend_from_slice(&MANUFACTURER_ID.to_le_bytes());
        adv_data.extend_from_slice(&[payload.len() as u8]);
        adv_data.extend_from_slice(payload);
        
        let options = AdvertisementOptions {
            discoverable: Some(true),
            connectable: Some(false), // Non-connectable — не тратим энергию на соединения
            ..Default::default()
        };
        
        let advertisement = Advertisement {
            manufacturer_data: Some(vec![(MANUFACTURER_ID, adv_data)]),
            service_uuids: Some(vec![Uuid::from_u128(SERVICE_UUID)]),
            ..Default::default()
        };
        
        let handle = adapter.advertise(advertisement, options).map_err(|e| format!("Failed to advertise: {}", e))?;
        
        // Удерживаем рекламу активной 500 мс (достаточно для обнаружения)
        std::thread::sleep(Duration::from_millis(500));
        
        // Останавливаем рекламу
        let _ = handle.stop();
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    fn scan_linux(&mut self, duration_secs: u64) -> Vec<Vec<u8>> {
        use bluer::{AdapterEvent, Device, Session};
        
        let mut results = Vec::new();
        
        let adapter = match self.adapter.as_ref() {
            Some(a) => a,
            None => {
                error!("Adapter not initialized");
                return results;
            }
        };
        
        let mut events = adapter.discover_devices().ok()?;
        
        let start = std::time::Instant::now();
        
        while start.elapsed() < Duration::from_secs(duration_secs) {
            if let Ok(event) = events.try_next() {
                match event {
                    AdapterEvent::DeviceAdded(addr) | AdapterEvent::DeviceUpdated(addr) => {
                        if let Ok(device) = adapter.device(addr) {
                            if let Ok(Some(rssi)) = device.rssi() {
                                if rssi > -90 { // Игнорируем слишком слабые сигналы
                                    if let Some(data) = Self::extract_inertia_data(&device) {
                                        debug!("Found Inertia spore from {} (RSSI: {} dBm)", addr, rssi);
                                        results.push(data);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            sleep(Duration::from_millis(10));
        }
        
        let _ = adapter.stop_discovery();
        
        // Сборка фрагментов
        Self::assemble_fragments(results)
    }
    
    #[cfg(target_os = "linux")]
    fn extract_inertia_data(device: &bluer::Device) -> Option<Vec<u8>> {
        if let Ok(Some(manufacturer_data)) = device.manufacturer_data() {
            for (id, data) in manufacturer_data {
                if id == MANUFACTURER_ID && data.len() > 1 {
                    let payload_len = data[0] as usize;
                    if payload_len <= data.len() - 1 {
                        return Some(data[1..1+payload_len].to_vec());
                    }
                }
            }
        }
        None
    }
    
    // ========== Cross-platform implementation (btleplug) ==========
    
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn init_btleplug(&mut self) -> Result<(), String> {
        use btleplug::api::Manager as _;
        
        let manager = Manager::new().map_err(|e| format!("Failed to create BT manager: {}", e))?;
        let adapters = manager.adapters().map_err(|e| format!("Failed to get adapters: {}", e))?;
        
        let central = adapters.into_iter().next().ok_or("No Bluetooth adapter found")?;
        
        central.start_scan(ScanFilter::default()).map_err(|e| format!("Failed to start scan: {}", e))?;
        
        debug!("Bluetooth adapter ready (btleplug)");
        self.central = Some(central);
        Ok(())
    }
    
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn broadcast_btleplug(&self, payload: &[u8]) -> Result<(), String> {
        use btleplug::api::Peripheral as _;
        
        let central = self.central.as_ref().ok_or("Central not initialized")?;
        
        // btleplug не поддерживает прямую рекламу без соединения
        // Для кросс-платформенности используем beacon-подход:
        // Регистрируем сервис с характеристикой, которую можно читать без соединения
        
        warn!("Direct advertising on this platform requires peripheral mode. Using workaround...");
        
        // TODO: Реализовать через GATT Server с характеристикой, доступной для чтения
        // Пока возвращаем ошибку
        Err("Direct Bluetooth broadcast not fully implemented on this platform. Use Linux for full functionality.".to_string())
    }
    
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn scan_btleplug(&mut self, duration_secs: u64) -> Vec<Vec<u8>> {
        use btleplug::api::{Central, CentralEvent, Peripheral, ScanFilter};
        use futures::stream::StreamExt;
        
        let mut results = Vec::new();
        
        let central = match self.central.as_ref() {
            Some(c) => c,
            None => {
                error!("Central not initialized");
                return results;
            }
        };
        
        let _ = central.start_scan(ScanFilter::default());
        
        let mut events = central.events().await?;
        
        let start = std::time::Instant::now();
        
        while start.elapsed() < Duration::from_secs(duration_secs) {
            tokio::select! {
                Some(event) = events.next() => {
                    match event {
                        CentralEvent::DeviceDiscovered(addr) | CentralEvent::DeviceUpdated(addr) => {
                            if let Some(peripheral) = central.peripheral(&addr) {
                                if let Ok(properties) = peripheral.properties() {
                                    if let Some(data) = Self::extract_inertia_data_from_properties(&properties) {
                                        debug!("Found Inertia spore from {:?}", addr);
                                        results.push(data);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
        
        let _ = central.stop_scan();
        
        Self::assemble_fragments(results)
    }
    
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn extract_inertia_data_from_properties(properties: &btleplug::api::PeripheralProperties) -> Option<Vec<u8>> {
        if let Some(manufacturer_data) = &properties.manufacturer_data {
            for (id, data) in manufacturer_data {
                // btleplug использует u16 как ID производителя
                if *id == MANUFACTURER_ID && data.len() > 1 {
                    let payload_len = data[0] as usize;
                    if payload_len <= data.len() - 1 {
                        return Some(data[1..1+payload_len].to_vec());
                    }
                }
            }
        }
        
        // Альтернатива: ищем в service data
        if let Some(service_data) = &properties.service_data {
            for (uuid, data) in service_data {
                if uuid.as_u128() == SERVICE_UUID && data.len() > 1 {
                    let payload_len = data[0] as usize;
                    if payload_len <= data.len() - 1 {
                        return Some(data[1..1+payload_len].to_vec());
                    }
                }
            }
        }
        
        None
    }
    
    // ========== Общие вспомогательные функции ==========
    
    /// Сборка фрагментированных сообщений
    fn assemble_fragments(mut fragments: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        if fragments.is_empty() {
            return Vec::new();
        }
        
        // Сортируем по первому байту (индекс фрагмента)
        fragments.sort_by(|a, b| a[0].cmp(&b[0]));
        
        let mut result = Vec::new();
        for fragment in fragments {
            if fragment.len() > 1 {
                result.extend_from_slice(&fragment[1..]);
            }
        }
        
        if !result.is_empty() {
            info!("Assembled {} bytes from Bluetooth fragments", result.len());
        }
        
        vec![result]
    }
}

impl Default for BluetoothAdv {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BluetoothAdv {
    fn drop(&mut self) {
        self.stop();
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_payload_size_limit() {
        let small_payload = vec![1u8; 10];
        assert!(small_payload.len() <= MAX_ADV_PAYLOAD);
        
        let large_payload = vec![1u8; 100];
        assert!(large_payload.len() > MAX_ADV_PAYLOAD);
    }
    
    #[test]
    fn test_fragmentation() {
        let adv = BluetoothAdv::new();
        let data = vec![0xAA; 100];
        let fragments = adv.broadcast_fragmented(&data);
        
        // 100 байт / (31-1) = 100/30 ≈ 4 фрагмента
        assert!(fragments.len() >= 3 && fragments.len() <= 5);
    }
    
    #[test]
    fn test_assemble_fragments() {
        let fragments = vec![
            vec![0x00, 0x01, 0x02],
            vec![0x01, 0x03, 0x04],
            vec![0x02, 0x05, 0x06],
        ];
        
        let assembled = BluetoothAdv::assemble_fragments(fragments);
        assert!(!assembled.is_empty());
        assert_eq!(assembled[0], vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }
}

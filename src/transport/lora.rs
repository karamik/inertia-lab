// src/transport/lora.rs
// LoRa транспорт для Inertia — дальность до 10 км
// Inertia Protocol — Post-Internet Digital Species

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use serialport::{SerialPort, SerialPortType, SerialPortInfo};
use log::{debug, info, warn, error};

// Константы LoRa
const LORA_FREQUENCY: u32 = 868_000_000;      // 868 МГц (EU) / 915 МГц (US)
const LORA_BANDWIDTH: u32 = 125_000;           // 125 кГц
const LORA_SPREADING_FACTOR: u8 = 9;           // SF9 (баланс скорости и дальности)
const LORA_CODING_RATE: u8 = 5;                // 4/5
const LORA_TX_POWER: i8 = 20;                  // 20 dBm (максимум)
const MAX_PACKET_SIZE: usize = 255;             // Максимальный байт в пакете LoRa
const MESH_HOPS_MAX: u8 = 7;                   // Максимум пересылок
const ROUTING_TABLE_SIZE: usize = 100;          // Маршрутов в таблице

/// Тип пакета LoRa
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoRaPacketType {
    Data = 0x01,        // Обычные данные
    Beacon = 0x02,      // Поисковый маяк
    Ack = 0x03,         // Подтверждение получения
    RouteRequest = 0x04, // Запрос маршрута
    RouteReply = 0x05,   // Ответ на маршрут
    Encounter = 0x06,    // Данные встречи PoE
    Block = 0x07,        // Блок блокчейна
}

/// Заголовок пакета LoRa
#[derive(Debug, Clone)]
pub struct LoRaHeader {
    pub packet_type: LoRaPacketType,
    pub src_id: [u8; 16],       // ID отправителя (первые 16 байт публичного ключа)
    pub dst_id: [u8; 16],       // ID получателя
    pub hop_count: u8,          // Текущее количество пересылок
    pub ttl: u8,                // Время жизни (оставшиеся пересылки)
    pub packet_id: u32,         // Уникальный ID пакета
    pub timestamp: u64,         // Время отправки
    pub payload_len: u16,       // Длина полезной нагрузки
}

/// Пакет LoRa
#[derive(Debug, Clone)]
pub struct LoRaPacket {
    pub header: LoRaHeader,
    pub payload: Vec<u8>,
}

/// Запись маршрута в таблице
#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub destination: [u8; 16],
    pub next_hop: [u8; 16],
    pub hops: u8,
    pub last_seen: u64,
    pub signal_quality: f32,    // RSSI среднее
}

/// Буферизованный пакет для ретрансляции
#[derive(Debug, Clone)]
pub struct BufferedPacket {
    pub packet: LoRaPacket,
    pub retry_count: u8,
    pub next_retry: u64,
}

/// LoRa транспорт с mesh-маршрутизацией
pub struct LoraTransport {
    port: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    node_id: [u8; 16],
    routing_table: Arc<Mutex<Vec<RouteEntry>>>,
    packet_buffer: Arc<Mutex<VecDeque<BufferedPacket>>>,
    received_packets: Arc<Mutex<Vec<LoRaPacket>>>,
    running: Arc<Mutex<bool>>,
    frequency: u32,
    spreading_factor: u8,
    tx_power: i8,
}

impl LoraTransport {
    pub fn new(node_id: [u8; 16]) -> Self {
        Self {
            port: Arc::new(Mutex::new(None)),
            node_id,
            routing_table: Arc::new(Mutex::new(Vec::new())),
            packet_buffer: Arc::new(Mutex::new(VecDeque::new())),
            received_packets: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(Mutex::new(false)),
            frequency: LORA_FREQUENCY,
            spreading_factor: LORA_SPREADING_FACTOR,
            tx_power: LORA_TX_POWER,
        }
    }

    /// Инициализация LoRa-модуля
    pub fn init(&mut self, port_name: &str) -> Result<(), String> {
        info!("Initializing LoRa module on {}", port_name);

        // Открываем последовательный порт
        let port = serialport::new(port_name, 115200)
            .timeout(Duration::from_millis(1000))
            .open()
            .map_err(|e| format!("Failed to open port {}: {}", port_name, e))?;

        *self.port.lock().unwrap() = Some(port);

        // Настройка LoRa-модуля (AT-команды)
        self.send_at_command("AT\r\n", "OK")?;
        self.send_at_command(&format!("AT+FREQ={}\r\n", self.frequency), "OK")?;
        self.send_at_command(&format!("AT+SF={}\r\n", self.spreading_factor), "OK")?;
        self.send_at_command(&format!("AT+POWER={}\r\n", self.tx_power), "OK")?;
        self.send_at_command("AT+CAD=ON\r\n", "OK")?;  // Активное обнаружение несущей

        info!("LoRa module initialized successfully");
        Ok(())
    }

    /// Запуск фонового приёма и обработки
    pub fn start(&mut self) -> Result<(), String> {
        info!("Starting LoRa transport...");
        *self.running.lock().unwrap() = true;

        let running = self.running.clone();
        let port = self.port.clone();
        let routing_table = self.routing_table.clone();
        let packet_buffer = self.packet_buffer.clone();
        let received_packets = self.received_packets.clone();
        let node_id = self.node_id;
        let frequency = self.frequency;
        let spreading_factor = self.spreading_factor;
        let tx_power = self.tx_power;

        // Запускаем поток приёма
        thread::spawn(move || {
            Self::receive_loop(
                running, port, routing_table, packet_buffer,
                received_packets, node_id, frequency, spreading_factor, tx_power
            );
        });

        // Запускаем поток обработки буфера (ретрансляция)
        let running2 = self.running.clone();
        let packet_buffer2 = self.packet_buffer.clone();
        let port2 = self.port.clone();
        let node_id2 = self.node_id;

        thread::spawn(move || {
            Self::retransmit_loop(running2, packet_buffer2, port2, node_id2);
        });

        info!("LoRa transport started");
        Ok(())
    }

    /// Остановка транспорта
    pub fn stop(&mut self) {
        info!("Stopping LoRa transport...");
        *self.running.lock().unwrap() = false;
    }

    /// Отправка данных
    pub fn send(&mut self, dst_id: [u8; 16], data: &[u8]) -> Result<(), String> {
        if data.len() > MAX_PACKET_SIZE - 40 {
            return Err("Data too large for LoRa packet".to_string());
        }

        let next_hop = self.find_next_hop(&dst_id);
        
        let header = LoRaHeader {
            packet_type: LoRaPacketType::Data,
            src_id: self.node_id,
            dst_id,
            hop_count: 0,
            ttl: MESH_HOPS_MAX,
            packet_id: self.next_packet_id(),
            timestamp: self.current_timestamp(),
            payload_len: data.len() as u16,
        };

        let packet = LoRaPacket {
            header,
            payload: data.to_vec(),
        };

        let target = if let Some(next) = next_hop {
            next
        } else {
            dst_id
        };

        self.send_packet(&packet, &target)?;
        debug!("LoRa packet sent to {:?}", hex::encode(&dst_id[..4]));
        Ok(())
    }

    /// Получение всех накопленных пакетов
    pub fn receive_all(&mut self) -> Vec<LoRaPacket> {
        let mut packets = self.received_packets.lock().unwrap();
        let result = packets.clone();
        packets.clear();
        result
    }

    /// Широковещательная рассылка
    pub fn broadcast(&mut self, data: &[u8]) -> Result<(), String> {
        let zero_id = [0u8; 16];
        self.send(zero_id, data)
    }

    // ========== Внутренние методы ==========

    fn send_at_command(&mut self, cmd: &str, expected: &str) -> Result<(), String> {
        let mut port = self.port.lock().unwrap();
        if let Some(ref mut p) = *port {
            p.write_all(cmd.as_bytes()).map_err(|e| e.to_string())?;
            p.flush().map_err(|e| e.to_string())?;
            
            let mut buf = [0u8; 256];
            let n = p.read(&mut buf).map_err(|e| e.to_string())?;
            let response = String::from_utf8_lossy(&buf[..n]);
            
            if response.contains(expected) {
                Ok(())
            } else {
                Err(format!("AT command failed: {}", response))
            }
        } else {
            Err("Port not initialized".to_string())
        }
    }

    fn send_packet(&mut self, packet: &LoRaPacket, target_id: &[u8; 16]) -> Result<(), String> {
        let encoded = self.encode_packet(packet);
        let mut port = self.port.lock().unwrap();
        
        if let Some(ref mut p) = *port {
            p.write_all(&encoded).map_err(|e| e.to_string())?;
            p.flush().map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Port not initialized".to_string())
        }
    }

    fn encode_packet(&self, packet: &LoRaPacket) -> Vec<u8> {
        let mut data = Vec::new();
        
        // Заголовок
        data.push(packet.header.packet_type as u8);
        data.extend_from_slice(&packet.header.src_id);
        data.extend_from_slice(&packet.header.dst_id);
        data.push(packet.header.hop_count);
        data.push(packet.header.ttl);
        data.extend_from_slice(&packet.header.packet_id.to_le_bytes());
        data.extend_from_slice(&packet.header.timestamp.to_le_bytes());
        data.extend_from_slice(&packet.header.payload_len.to_le_bytes());
        
        // Данные
        data.extend_from_slice(&packet.payload);
        
        data
    }

    fn decode_packet(data: &[u8]) -> Option<LoRaPacket> {
        if data.len() < 40 {
            return None;
        }
        
        let mut offset = 0;
        let packet_type = match data[offset] {
            0x01 => LoRaPacketType::Data,
            0x02 => LoRaPacketType::Beacon,
            0x03 => LoRaPacketType::Ack,
            0x04 => LoRaPacketType::RouteRequest,
            0x05 => LoRaPacketType::RouteReply,
            0x06 => LoRaPacketType::Encounter,
            0x07 => LoRaPacketType::Block,
            _ => return None,
        };
        offset += 1;
        
        let mut src_id = [0u8; 16];
        src_id.copy_from_slice(&data[offset..offset+16]);
        offset += 16;
        
        let mut dst_id = [0u8; 16];
        dst_id.copy_from_slice(&data[offset..offset+16]);
        offset += 16;
        
        let hop_count = data[offset];
        offset += 1;
        
        let ttl = data[offset];
        offset += 1;
        
        let packet_id = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
        offset += 4;
        
        let timestamp = u64::from_le_bytes(data[offset..offset+8].try_into().unwrap());
        offset += 8;
        
        let payload_len = u16::from_le_bytes(data[offset..offset+2].try_into().unwrap());
        offset += 2;
        
        let payload = data[offset..offset+payload_len as usize].to_vec();
        
        Some(LoRaPacket {
            header: LoRaHeader {
                packet_type,
                src_id,
                dst_id,
                hop_count,
                ttl,
                packet_id,
                timestamp,
                payload_len,
            },
            payload,
        })
    }

    fn find_next_hop(&self, dst_id: &[u8; 16]) -> Option<[u8; 16]> {
        let table = self.routing_table.lock().unwrap();
        for entry in table.iter() {
            if entry.destination == *dst_id {
                return Some(entry.next_hop);
            }
        }
        None
    }

    fn update_routing_table(&self, src_id: [u8; 16], next_hop: [u8; 16], hops: u8, rssi: f32) {
        let mut table = self.routing_table.lock().unwrap();
        let now = self.current_timestamp();
        
        if let Some(entry) = table.iter_mut().find(|e| e.destination == src_id) {
            entry.next_hop = next_hop;
            entry.hops = hops;
            entry.last_seen = now;
            entry.signal_quality = (entry.signal_quality * 0.7 + rssi * 0.3).max(-120.0);
        } else {
            table.push(RouteEntry {
                destination: src_id,
                next_hop,
                hops,
                last_seen: now,
                signal_quality: rssi,
            });
            
            if table.len() > ROUTING_TABLE_SIZE {
                // Удаляем самую старую запись
                table.sort_by(|a, b| a.last_seen.cmp(&b.last_seen));
                table.remove(0);
            }
        }
    }

    fn forward_packet(&self, packet: &LoRaPacket, rssi: f32) {
        let mut new_packet = packet.clone();
        new_packet.header.hop_count += 1;
        new_packet.header.ttl -= 1;
        
        if new_packet.header.ttl == 0 {
            return;
        }
        
        // Если это не наш пакет и не для нас — ретранслируем
        if new_packet.header.dst_id != self.node_id {
            let next_hop = self.find_next_hop(&new_packet.header.dst_id);
            let target = next_hop.unwrap_or(new_packet.header.dst_id);
            
            let mut buffer = self.packet_buffer.lock().unwrap();
            buffer.push_back(BufferedPacket {
                packet: new_packet,
                retry_count: 0,
                next_retry: self.current_timestamp(),
            });
        }
    }

    fn receive_loop(
        running: Arc<Mutex<bool>>,
        port: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
        routing_table: Arc<Mutex<Vec<RouteEntry>>>,
        packet_buffer: Arc<Mutex<VecDeque<BufferedPacket>>>,
        received_packets: Arc<Mutex<Vec<LoRaPacket>>>,
        node_id: [u8; 16],
        _frequency: u32,
        _spreading_factor: u8,
        _tx_power: i8,
    ) {
        let mut buf = [0u8; 512];
        
        while *running.lock().unwrap() {
            let mut port_guard = port.lock().unwrap();
            if let Some(ref mut p) = *port_guard {
                match p.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        if let Some(packet) = Self::decode_packet(&buf[..n]) {
                            let rssi = -45.0; // В реальности из драйвера
                            
                            // Обновляем маршрутную таблицу
                            let mut table = routing_table.lock().unwrap();
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            table.push(RouteEntry {
                                destination: packet.header.src_id,
                                next_hop: packet.header.src_id,
                                hops: packet.header.hop_count + 1,
                                last_seen: now,
                                signal_quality: rssi,
                            });
                            
                            // Если пакет для нас — сохраняем
                            if packet.header.dst_id == node_id {
                                received_packets.lock().unwrap().push(packet);
                            } else {
                                // Ретранслируем
                                let mut buffer = packet_buffer.lock().unwrap();
                                buffer.push_back(BufferedPacket {
                                    packet,
                                    retry_count: 0,
                                    next_retry: now,
                                });
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("LoRa read error: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn retransmit_loop(
        running: Arc<Mutex<bool>>,
        packet_buffer: Arc<Mutex<VecDeque<BufferedPacket>>>,
        port: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
        node_id: [u8; 16],
    ) {
        while *running.lock().unwrap() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let mut buffer = packet_buffer.lock().unwrap();
            let to_send: Vec<BufferedPacket> = buffer
                .drain(..)
                .filter(|b| b.next_retry <= now)
                .collect();
            
            drop(buffer);
            
            for mut buffered in to_send {
                let encoded = Self::encode_packet_static(&buffered.packet);
                let mut port_guard = port.lock().unwrap();
                if let Some(ref mut p) = *port_guard {
                    if p.write_all(&encoded).is_ok() {
                        p.flush().ok();
                    } else if buffered.retry_count < 3 {
                        buffered.retry_count += 1;
                        buffered.next_retry = now + (1 << buffered.retry_count);
                        packet_buffer.lock().unwrap().push_back(buffered);
                    }
                }
            }
            
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn encode_packet_static(packet: &LoRaPacket) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(packet.header.packet_type as u8);
        data.extend_from_slice(&packet.header.src_id);
        data.extend_from_slice(&packet.header.dst_id);
        data.push(packet.header.hop_count);
        data.push(packet.header.ttl);
        data.extend_from_slice(&packet.header.packet_id.to_le_bytes());
        data.extend_from_slice(&packet.header.timestamp.to_le_bytes());
        data.extend_from_slice(&packet.header.payload_len.to_le_bytes());
        data.extend_from_slice(&packet.payload);
        data
    }

    fn next_packet_id(&self) -> u32 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        NEXT_ID.fetch_add(1, Ordering::SeqCst)
    }

    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for LoraTransport {
    fn default() -> Self {
        Self::new([0u8; 16])
    }
}

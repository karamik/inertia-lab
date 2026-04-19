// src/transport/wifi_stego.rs
// Модуль для скрытой передачи данных через SSID (имена Wi-Fi сетей)
// Inertia Protocol — Post-Internet Digital Species

use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use log::{debug, info, warn, error};
use base32;

const SSID_PREFIX: &str = "IN_";
const MAX_SSID_LEN: usize = 32;
const PAYLOAD_CHUNK_SIZE: usize = 20; // байт на фрагмент

pub struct WifiStego;

impl WifiStego {
    /// Передача одного фрагмента данных через SSID
    pub fn broadcast(payload: &[u8]) -> Result<(), String> {
        if payload.is_empty() {
            return Err("Empty payload".to_string());
        }

        let encoded = base32::encode(base32::Alphabet::RFC4648 { padding: false }, payload);
        let ssid = format!("{}{}", SSID_PREFIX, encoded);
        
        let safe_ssid = if ssid.len() > MAX_SSID_LEN {
            warn!("SSID truncated from {} to {} chars", ssid.len(), MAX_SSID_LEN);
            &ssid[..MAX_SSID_LEN]
        } else {
            &ssid
        };

        debug!("Broadcasting via SSID: {}", safe_ssid);
        
        #[cfg(target_os = "linux")]
        return Self::set_ssid_linux(safe_ssid);
        
        #[cfg(target_os = "macos")]
        return Self::set_ssid_macos(safe_ssid);
        
        #[cfg(target_os = "windows")]
        return Self::set_ssid_windows(safe_ssid);
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err("Unsupported OS".to_string());
    }

    /// Передача больших данных с автоматической фрагментацией
    pub fn broadcast_fragmented(payload: &[u8]) -> Vec<Result<(), String>> {
        payload.chunks(PAYLOAD_CHUNK_SIZE)
            .enumerate()
            .map(|(i, chunk)| {
                let mut fragment = Vec::with_capacity(chunk.len() + 1);
                fragment.push(i as u8); // номер фрагмента (0-255)
                fragment.extend_from_slice(chunk);
                Self::broadcast(&fragment)
            })
            .collect()
    }

    /// Сканирование эфира и сбор фрагментов
    pub fn scan() -> Vec<Vec<u8>> {
        debug!("Scanning for Inertia spores in Wi-Fi names...");
        
        #[cfg(target_os = "linux")]
        let networks = Self::scan_linux();
        
        #[cfg(target_os = "macos")]
        let networks = Self::scan_macos();
        
        #[cfg(target_os = "windows")]
        let networks = Self::scan_windows();
        
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let networks = Vec::new();
        
        // Сборка фрагментов по индексам
        let mut fragments: Vec<(u8, Vec<u8>)> = networks.into_iter()
            .filter_map(|data| {
                if data.is_empty() { return None; }
                let idx = data[0];
                let payload = data[1..].to_vec();
                Some((idx, payload))
            })
            .collect();
        
        fragments.sort_by_key(|(idx, _)| *idx);
        
        // Склейка
        let mut result = Vec::new();
        for (_, payload) in fragments {
            result.extend_from_slice(&payload);
        }
        
        if !result.is_empty() {
            info!("Assembled {} bytes from Wi-Fi spores", result.len());
        }
        
        vec![result]
    }

    // ========== Platform-specific implementations ==========
    
    #[cfg(target_os = "linux")]
    fn set_ssid_linux(ssid: &str) -> Result<(), String> {
        // Вариант 1: через iw (не разрывает соединение)
        let status = Command::new("iw")
            .args(&["dev", "wlan0", "set", "mesh", "ssid", ssid])
            .status();
        
        if let Ok(status) = status {
            if status.success() {
                return Ok(());
            }
        }
        
        // Вариант 2: fallback на nmcli
        let status = Command::new("nmcli")
            .args(&["device", "wifi", "hotspot", "ssid", ssid])
            .status()
            .map_err(|e| e.to_string())?;
        
        if status.success() {
            Ok(())
        } else {
            Err("Failed to set SSID via iw or nmcli".to_string())
        }
    }
    
    #[cfg(target_os = "linux")]
    fn scan_linux() -> Vec<Vec<u8>> {
        // Принудительное сканирование
        let _ = Command::new("nmcli")
            .args(&["device", "wifi", "rescan"])
            .status();
        
        sleep(Duration::from_millis(2000));
        
        let output = Command::new("nmcli")
            .args(&["-t", "-f", "SSID", "device", "wifi", "list"])
            .output()
            .map_err(|e| error!("nmcli failed: {}", e))
            .ok()?;
        
        let networks = String::from_utf8_lossy(&output.stdout);
        
        networks.lines()
            .filter(|line| line.starts_with(SSID_PREFIX))
            .filter_map(|line| {
                let data = &line[SSID_PREFIX.len()..];
                base32::decode(base32::Alphabet::RFC4648 { padding: false }, data)
            })
            .collect()
    }
    
    #[cfg(target_os = "macos")]
    fn set_ssid_macos(ssid: &str) -> Result<(), String> {
        let status = Command::new("networksetup")
            .args(&["-setairportnetwork", "en0", ssid])
            .status()
            .map_err(|e| e.to_string())?;
        
        if status.success() {
            Ok(())
        } else {
            Err("Failed to set SSID on macOS".to_string())
        }
    }
    
    #[cfg(target_os = "macos")]
    fn scan_macos() -> Vec<Vec<u8>> {
        let output = Command::new("/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport")
            .args(&["-s"])
            .output()
            .ok()?;
        
        let networks = String::from_utf8_lossy(&output.stdout);
        
        networks.lines()
            .filter(|line| line.contains(SSID_PREFIX))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let ssid = parts.first()?;
                if ssid.starts_with(SSID_PREFIX) {
                    let data = &ssid[SSID_PREFIX.len()..];
                    base32::decode(base32::Alphabet::RFC4648 { padding: false }, data)
                } else {
                    None
                }
            })
            .collect()
    }
    
    #[cfg(target_os = "windows")]
    fn set_ssid_windows(ssid: &str) -> Result<(), String> {
        let status = Command::new("netsh")
            .args(&["wlan", "set", "hostednetwork", "ssid=", ssid])
            .status()
            .map_err(|e| e.to_string())?;
        
        if status.success() {
            Ok(())
        } else {
            Err("Failed to set SSID on Windows".to_string())
        }
    }
    
    #[cfg(target_os = "windows")]
    fn scan_windows() -> Vec<Vec<u8>> {
        let output = Command::new("netsh")
            .args(&["wlan", "show", "networks", "mode=bssid"])
            .output()
            .ok()?;
        
        let networks = String::from_utf8_lossy(&output.stdout);
        
        networks.lines()
            .filter(|line| line.contains("SSID") && line.contains(SSID_PREFIX))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(':').collect();
                let ssid = parts.last()?.trim();
                if ssid.starts_with(SSID_PREFIX) {
                    let data = &ssid[SSID_PREFIX.len()..];
                    base32::decode(base32::Alphabet::RFC4648 { padding: false }, data)
                } else {
                    None
                }
            })
            .collect()
    }
}

// ========== Unit Tests ==========

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode() {
        let original = b"Hello, Inertia!";
        let encoded = base32::encode(base32::Alphabet::RFC4648 { padding: false }, original);
        let decoded = base32::decode(base32::Alphabet::RFC4648 { padding: false }, &encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }
    
    #[test]
    fn test_fragmentation() {
        let data = vec![0u8; 100];
        let fragments = WifiStego::broadcast_fragmented(&data);
        assert_eq!(fragments.len(), 5); // 100 / 20 = 5 фрагментов
    }
}

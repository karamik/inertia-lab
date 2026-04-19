// src/consensus/astro_anchor.rs
// Астрономический якорь — верификация через звёздное небо
// Inertia Protocol — Post-Internet Digital Species
//
// Астрономический якорь — самый надёжный источник истины:
// 1. Звёзды невозможно подделать, отключить или заблокировать
// 2. Положение звёзд уникально для каждого момента времени и места
// 3. Не требует GPS, NTP или интернета
// 4. Работает на любом устройстве с камерой
// 5. Обеспечивает "In Physics We Trust"

use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::f64::consts::PI;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use blake3::Hash;
use log::{debug, info, warn, error};

// Константы астрономического якоря
const ASTRO_ANCHOR_VERSION: u8 = 1;
const MIN_STARS_FOR_ANCHOR: usize = 3;      // Минимум 3 звезды для верификации
const MAX_STARS_FOR_ANCHOR: usize = 7;      // Максимум 7 звёзд в якоре
const ASTRO_TIMESTAMP_TOLERANCE_SECS: u64 = 300; // 5 минут допустимого расхождения
const STELLAR_MAGNITUDE_LIMIT: f64 = 4.0;   // Звёзды ярче 4m (видимые невооружённым глазом)

/// Каталог ярких звёзд (Hipparcos)
#[derive(Debug, Clone, Copy)]
pub struct Star {
    pub hip_id: u32,            // Hipparcos ID
    pub name: &'static str,     // Общепринятое название
    pub ra_hours: f64,          // Прямое восхождение (часы)
    pub dec_degrees: f64,       // Склонение (градусы)
    pub magnitude: f64,         // Видимая звёздная величина
    pub color_index: f64,       // Цветовой индекс B-V
}

/// Каталог навигационных звёзд (самые яркие)
const NAVIGATION_STARS: &[Star] = &[
    Star { hip_id: 32349, name: "Sirius", ra_hours: 6.7525, dec_degrees: -16.7161, magnitude: -1.46, color_index: 0.01 },
    Star { hip_id: 30438, name: "Canopus", ra_hours: 6.3999, dec_degrees: -52.6956, magnitude: -0.72, color_index: 0.15 },
    Star { hip_id: 71683, name: "Arcturus", ra_hours: 14.2667, dec_degrees: 19.1825, magnitude: -0.05, color_index: 1.23 },
    Star { hip_id: 91262, name: "Vega", ra_hours: 18.6156, dec_degrees: 38.7837, magnitude: 0.03, color_index: 0.00 },
    Star { hip_id: 24436, name: "Capella", ra_hours: 5.2778, dec_degrees: 45.9980, magnitude: 0.08, color_index: 0.80 },
    Star { hip_id: 69673, name: "Rigel", ra_hours: 5.2422, dec_degrees: -8.2016, magnitude: 0.13, color_index: -0.03 },
    Star { hip_id: 37279, name: "Procyon", ra_hours: 7.6556, dec_degrees: 5.2250, magnitude: 0.34, color_index: 0.42 },
    Star { hip_id: 60718, name: "Betelgeuse", ra_hours: 5.9197, dec_degrees: 7.4072, magnitude: 0.42, color_index: 1.85 },
    Star { hip_id: 21421, name: "Aldebaran", ra_hours: 4.6142, dec_degrees: 16.5092, magnitude: 0.85, color_index: 1.54 },
    Star { hip_id: 25428, name: "Spica", ra_hours: 13.4167, dec_degrees: -11.1614, magnitude: 0.98, color_index: -0.23 },
    Star { hip_id: 113368, name: "Antares", ra_hours: 16.4936, dec_degrees: -26.4320, magnitude: 1.06, color_index: 1.83 },
    Star { hip_id: 102098, name: "Fomalhaut", ra_hours: 22.9547, dec_degrees: -29.6225, magnitude: 1.17, color_index: 0.09 },
    Star { hip_id: 85927, name: "Deneb", ra_hours: 20.6985, dec_degrees: 45.2803, magnitude: 1.25, color_index: 0.09 },
    Star { hip_id: 78820, name: "Regulus", ra_hours: 10.1602, dec_degrees: 11.9672, magnitude: 1.36, color_index: -0.11 },
    Star { hip_id: 11767, name: "Pollux", ra_hours: 7.7703, dec_degrees: 28.0262, magnitude: 1.16, color_index: 1.07 },
];

/// Результат распознавания звёзд на изображении
#[derive(Debug, Clone)]
pub struct DetectedStar {
    pub star: Star,
    pub x: f64,                 // Координата X на изображении (пиксели)
    pub y: f64,                 // Координата Y на изображении (пиксели)
    pub brightness: f64,        // Яркость в цифрах
    pub confidence: f64,        // Уверенность распознавания (0-1)
}

/// Астрономический якорь — криптографическая подпись неба
#[derive(Debug, Clone)]
pub struct AstronomicalAnchor {
    pub version: u8,
    pub timestamp: u64,                     // Unix timestamp
    pub latitude: f64,                      // Широта (градусы, -90..90)
    pub longitude: f64,                     // Долгота (градусы, -180..180)
    pub stars: Vec<Star>,                   // Опознанные звёзды
    pub star_positions: Vec<(f64, f64)>,    // Позиции звёзд на небе (az, alt)
    pub camera_hash: [u8; 32],              // Хеш параметров камеры
    pub image_hash: [u8; 32],               // Хеш изображения (для верификации)
    pub signature: [u8; 64],                // Подпись якоря
}

/// Параметры камеры для астрометрии
#[derive(Debug, Clone)]
pub struct CameraParameters {
    pub focal_length_mm: f64,       // Фокусное расстояние (мм)
    pub sensor_width_mm: f64,       // Ширина сенсора (мм)
    pub sensor_height_mm: f64,      // Высота сенсора (мм)
    pub image_width: u32,           // Ширина изображения (пиксели)
    pub image_height: u32,          // Высота изображения (пиксели)
    pub distortion_k1: f64,         // Радиальное искажение (параметр 1)
    pub distortion_k2: f64,         // Радиальное искажение (параметр 2)
}

impl Default for CameraParameters {
    fn default() -> Self {
        Self {
            focal_length_mm: 4.0,       // Типичный смартфон
            sensor_width_mm: 5.6,
            sensor_height_mm: 4.2,
            image_width: 4032,
            image_height: 3024,
            distortion_k1: 0.0,
            distortion_k2: 0.0,
        }
    }
}

/// Астрономический верификатор
pub struct AstroAnchor {
    camera_params: CameraParameters,
    star_catalog: Vec<Star>,
    anchors: Arc<Mutex<HashMap<[u8; 32], AstronomicalAnchor>>>,
}

impl AstroAnchor {
    pub fn new() -> Self {
        Self {
            camera_params: CameraParameters::default(),
            star_catalog: NAVIGATION_STARS.to_vec(),
            anchors: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub fn with_camera_params(params: CameraParameters) -> Self {
        Self {
            camera_params: params,
            star_catalog: NAVIGATION_STARS.to_vec(),
            anchors: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Создание астрономического якоря из изображения звёздного неба
    pub fn create_anchor(
        &mut self,
        image_data: &[u8],
        latitude: f64,
        longitude: f64,
        private_key: &ed25519_dalek::Keypair,
    ) -> Result<AstronomicalAnchor, String> {
        
        info!("Creating astronomical anchor at ({:.4}, {:.4})", latitude, longitude);
        
        // 1. Распознаём звёзды на изображении
        let detected_stars = self.detect_stars(image_data)?;
        
        if detected_stars.len() < MIN_STARS_FOR_ANCHOR {
            return Err(format!("Only {} stars detected, need at least {}", 
                              detected_stars.len(), MIN_STARS_FOR_ANCHOR));
        }
        
        // 2. Идентифицируем звёзды по каталогу
        let identified_stars = self.identify_stars(&detected_stars)?;
        
        // 3. Вычисляем позиции звёзд на небе (азимут, высота)
        let timestamp = self.current_timestamp();
        let star_positions = self.compute_star_positions(&identified_stars, timestamp, latitude, longitude);
        
        // 4. Создаём хеш изображения
        let image_hash = *blake3::hash(image_data).as_bytes();
        
        // 5. Создаём хеш параметров камеры
        let camera_hash = self.hash_camera_params();
        
        // 6. Создаём якорь
        let anchor = AstronomicalAnchor {
            version: ASTRO_ANCHOR_VERSION,
            timestamp,
            latitude,
            longitude,
            stars: identified_stars,
            star_positions,
            camera_hash,
            image_hash,
            signature: [0u8; 64],
        };
        
        // 7. Подписываем якорь
        let signature_data = self.encode_anchor_for_signing(&anchor);
        let signature = private_key.sign(&signature_data);
        let mut anchor_signed = anchor;
        anchor_signed.signature.copy_from_slice(signature.as_bytes());
        
        // 8. Сохраняем якорь
        let anchor_id = self.hash_anchor(&anchor_signed);
        self.anchors.lock().unwrap().insert(anchor_id, anchor_signed.clone());
        
        info!("Astronomical anchor created with ID: {}", hex::encode(&anchor_id[..8]));
        
        Ok(anchor_signed)
    }
    
    /// Верификация астрономического якоря
    pub fn verify_anchor(&self, anchor: &AstronomicalAnchor) -> bool {
        // 1. Проверка версии
        if anchor.version != ASTRO_ANCHOR_VERSION {
            warn!("Invalid anchor version: {}", anchor.version);
            return false;
        }
        
        // 2. Проверка временной метки (не старше 5 минут)
        let now = self.current_timestamp();
        let age = now.saturating_sub(anchor.timestamp);
        if age > ASTRO_TIMESTAMP_TOLERANCE_SECS {
            warn!("Anchor too old: {} seconds", age);
            return false;
        }
        
        // 3. Проверка количества звёзд
        if anchor.stars.len() < MIN_STARS_FOR_ANCHOR {
            warn!("Too few stars: {}", anchor.stars.len());
            return false;
        }
        
        // 4. Проверка позиций звёзд (астрономическая консистенция)
        if !self.verify_star_positions(anchor) {
            warn!("Star positions inconsistent");
            return false;
        }
        
        // 5. Проверка, что звёзды видны в указанное время и месте
        if !self.verify_stars_visibility(anchor) {
            warn!("Stars not visible at given time/location");
            return false;
        }
        
        info!("Astronomical anchor verified successfully");
        true
    }
    
    /// Получение текущего времени из звёзд (без GPS/NTP)
    pub fn get_time_from_stars(&self, anchor: &AstronomicalAnchor) -> Option<u64> {
        // По положению звёзд вычисляем звёздное время
        let sidereal_time = self.compute_sidereal_time_from_stars(anchor);
        
        // Преобразуем звёздное время в UTC (с погрешностью)
        let utc_estimate = self.sidereal_to_utc(sidereal_time, anchor.longitude);
        
        // Проверяем, что результат в разумных пределах
        let now = self.current_timestamp();
        let diff = now.saturating_sub(utc_estimate);
        
        if diff < 3600 { // Погрешность меньше часа
            Some(utc_estimate)
        } else {
            None
        }
    }
    
    /// Получение геопозиции из звёзд (без GPS)
    pub fn get_position_from_stars(&self, anchor: &AstronomicalAnchor) -> Option<(f64, f64)> {
        // По высоте звёзд над горизонтом вычисляем широту
        let latitude = self.compute_latitude_from_stars(anchor);
        
        // По азимуту и времени вычисляем долготу
        let longitude = self.compute_longitude_from_stars(anchor);
        
        Some((latitude, longitude))
    }
    
    // ========== Распознавание звёзд (упрощённая версия) ==========
    
    fn detect_stars(&self, image_data: &[u8]) -> Result<Vec<DetectedStar>, String> {
        // В реальной реализации здесь используется OpenCV
        // Для прототипа — симуляция на основе тестовых данных
        
        debug!("Detecting stars in image ({} bytes)", image_data.len());
        
        // TODO: Интеграция с OpenCV
        // Для демо возвращаем тестовые данные
        
        Ok(vec![
            DetectedStar { star: NAVIGATION_STARS[0], x: 100.0, y: 200.0, brightness: 0.95, confidence: 0.98 },
            DetectedStar { star: NAVIGATION_STARS[2], x: 300.0, y: 150.0, brightness: 0.88, confidence: 0.95 },
            DetectedStar { star: NAVIGATION_STARS[3], x: 500.0, y: 400.0, brightness: 0.92, confidence: 0.97 },
        ])
    }
    
    fn identify_stars(&self, detected: &[DetectedStar]) -> Result<Vec<Star>, String> {
        // По паттерну созвездий идентифицируем звёзды
        // Для прототипа просто возвращаем звёзды из detected
        
        Ok(detected.iter().map(|d| d.star.clone()).collect())
    }
    
    // ========== Астрометрические вычисления ==========
    
    fn compute_star_positions(
        &self,
        stars: &[Star],
        timestamp: u64,
        latitude: f64,
        longitude: f64,
    ) -> Vec<(f64, f64)> {
        let jd = self.unix_to_julian_date(timestamp);
        
        stars.iter().map(|star| {
            let (az, alt) = self.star_to_az_alt(star, jd, latitude, longitude);
            (az, alt)
        }).collect()
    }
    
    fn star_to_az_alt(&self, star: &Star, jd: f64, latitude: f64, longitude: f64) -> (f64, f64) {
        // Преобразование экваториальных координат в горизонтальные
        
        // 1. Прямое восхождение в радианы
        let ra_rad = star.ra_hours * 15.0 * PI / 180.0;
        let dec_rad = star.dec_degrees * PI / 180.0;
        
        // 2. Звёздное время
        let lst = self.compute_local_sidereal_time(jd, longitude);
        let hour_angle = lst - ra_rad;
        
        // 3. Высота и азимут
        let lat_rad = latitude * PI / 180.0;
        let sin_alt = lat_rad.sin() * dec_rad.sin() + 
                      lat_rad.cos() * dec_rad.cos() * hour_angle.cos();
        let alt = sin_alt.asin();
        
        let cos_az = (dec_rad.sin() - lat_rad.sin() * sin_alt) / (lat_rad.cos() * alt.cos());
        let az = cos_az.acos();
        
        (az * 180.0 / PI, alt * 180.0 / PI)
    }
    
    fn compute_local_sidereal_time(&self, jd: f64, longitude: f64) -> f64 {
        // Гринвичское звёздное время
        let t = (jd - 2451545.0) / 36525.0;
        let gmst = 280.46061837 + 360.98564736629 * (jd - 2451545.0) + 
                   0.000387933 * t * t - t * t * t / 38710000.0;
        
        // Местное звёздное время
        let lst = gmst + longitude;
        lst * PI / 180.0
    }
    
    fn unix_to_julian_date(&self, unix_secs: u64) -> f64 {
        (unix_secs as f64 / 86400.0) + 2440587.5
    }
    
    // ========== Верификация ==========
    
    fn verify_star_positions(&self, anchor: &AstronomicalAnchor) -> bool {
        // Проверяем, что звёзды находятся на ожидаемых позициях
        let jd = self.unix_to_julian_date(anchor.timestamp);
        
        for (i, star) in anchor.stars.iter().enumerate() {
            let (expected_az, expected_alt) = self.star_to_az_alt(
                star, jd, anchor.latitude, anchor.longitude
            );
            
            let (actual_az, actual_alt) = anchor.star_positions[i];
            
            let az_diff = (expected_az - actual_az).abs();
            let alt_diff = (expected_alt - actual_alt).abs();
            
            // Допустимая погрешность — 0.5 градуса
            if az_diff > 0.5 || alt_diff > 0.5 {
                warn!("Star {} position mismatch: az diff {:.2}°, alt diff {:.2}°",
                      star.name, az_diff, alt_diff);
                return false;
            }
        }
        
        true
    }
    
    fn verify_stars_visibility(&self, anchor: &AstronomicalAnchor) -> bool {
        // Проверяем, что звёзды находятся над горизонтом
        for (_, alt) in &anchor.star_positions {
            if *alt < 0.0 {
                warn!("Star below horizon (alt={:.1}°)", alt);
                return false;
            }
        }
        true
    }
    
    fn compute_sidereal_time_from_stars(&self, anchor: &AstronomicalAnchor) -> f64 {
        // По положению звёзд вычисляем звёздное время
        if anchor.stars.is_empty() {
            return 0.0;
        }
        
        let star = &anchor.stars[0];
        let (az, alt) = anchor.star_positions[0];
        
        // Обратная задача: из горизонтальных координат в часовой угол
        let lat_rad = anchor.latitude * PI / 180.0;
        let alt_rad = alt * PI / 180.0;
        let az_rad = az * PI / 180.0;
        let dec_rad = star.dec_degrees * PI / 180.0;
        
        let sin_h = (alt_rad.sin() - lat_rad.sin() * dec_rad.sin()) / 
                    (lat_rad.cos() * dec_rad.cos());
        let hour_angle = sin_h.asin();
        
        let ra_rad = star.ra_hours * 15.0 * PI / 180.0;
        let lst = ra_rad + hour_angle;
        
        lst
    }
    
    fn sidereal_to_utc(&self, sidereal: f64, longitude: f64) -> u64 {
        // Упрощённое преобразование звёздного времени в UTC
        let lst_deg = sidereal * 180.0 / PI;
        let gmst_deg = lst_deg - longitude;
        let days_since_epoch = gmst_deg / 360.985647;
        
        (2440587.5 + days_since_epoch) as u64 * 86400
    }
    
    fn compute_latitude_from_stars(&self, anchor: &AstronomicalAnchor) -> f64 {
        // Высота Полярной звезды ≈ широта (в северном полушарии)
        let polaris = anchor.stars.iter().find(|s| s.name == "Polaris");
        
        if let Some(polaris) = polaris {
            for (star, (_, alt)) in anchor.stars.iter().zip(&anchor.star_positions) {
                if star.name == polaris.name {
                    return alt;
                }
            }
        }
        
        // Fallback: по высоте звезды на меридиане
        if !anchor.stars.is_empty() && !anchor.star_positions.is_empty() {
            return anchor.star_positions[0].1;
        }
        
        0.0
    }
    
    fn compute_longitude_from_stars(&self, anchor: &AstronomicalAnchor) -> f64 {
        // Разница между местным и гринвичским звёздным временем
        let lst = self.compute_sidereal_time_from_stars(anchor);
        let gmst = self.compute_greenwich_sidereal_time(anchor.timestamp);
        
        let longitude = (lst - gmst) * 180.0 / PI;
        
        if longitude > 180.0 {
            longitude - 360.0
        } else {
            longitude
        }
    }
    
    fn compute_greenwich_sidereal_time(&self, timestamp: u64) -> f64 {
        let jd = self.unix_to_julian_date(timestamp);
        let t = (jd - 2451545.0) / 36525.0;
        let gmst = 280.46061837 + 360.98564736629 * (jd - 2451545.0) + 
                   0.000387933 * t * t - t * t * t / 38710000.0;
        gmst * PI / 180.0
    }
    
    // ========== Хеширование ==========
    
    fn hash_camera_params(&self) -> [u8; 32] {
        let data = format!(
            "{},{},{},{},{},{},{}",
            self.camera_params.focal_length_mm,
            self.camera_params.sensor_width_mm,
            self.camera_params.sensor_height_mm,
            self.camera_params.image_width,
            self.camera_params.image_height,
            self.camera_params.distortion_k1,
            self.camera_params.distortion_k2,
        );
        *blake3::hash(data.as_bytes()).as_bytes()
    }
    
    fn hash_anchor(&self, anchor: &AstronomicalAnchor) -> [u8; 32] {
        let data = self.encode_anchor_for_signing(anchor);
        *blake3::hash(&data).as_bytes()
    }
    
    fn encode_anchor_for_signing(&self, anchor: &AstronomicalAnchor) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(anchor.version);
        data.extend_from_slice(&anchor.timestamp.to_le_bytes());
        data.extend_from_slice(&anchor.latitude.to_le_bytes());
        data.extend_from_slice(&anchor.longitude.to_le_bytes());
        data.extend_from_slice(&anchor.camera_hash);
        data.extend_from_slice(&anchor.image_hash);
        
        for star in &anchor.stars {
            data.extend_from_slice(&star.hip_id.to_le_bytes());
        }
        
        data
    }
    
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

/// Интеграция астрономического якоря с Proof of Encounter
pub struct AstroPoE {
    astro: AstroAnchor,
    last_anchor: Option<AstronomicalAnchor>,
    verification_count: usize,
}

impl AstroPoE {
    pub fn new() -> Self {
        Self {
            astro: AstroAnchor::new(),
            last_anchor: None,
            verification_count: 0,
        }
    }
    
    /// Периодическая верификация через звёзды
    pub fn verify_with_stars(&mut self, image_data: &[u8], keypair: &ed25519_dalek::Keypair) -> bool {
        // Определяем примерную геопозицию (из последних встреч)
        let latitude = 55.7558;   // Для демо — Москва
        let longitude = 37.6176;
        
        match self.astro.create_anchor(image_data, latitude, longitude, keypair) {
            Ok(anchor) => {
                let valid = self.astro.verify_anchor(&anchor);
                if valid {
                    self.last_anchor = Some(anchor);
                    self.verification_count += 1;
                    info!("Star verification successful (count: {})", self.verification_count);
                }
                valid
            }
            Err(e) => {
                error!("Star verification failed: {}", e);
                false
            }
        }
    }
    
    /// Получение времени из звёзд (без GPS)
    pub fn get_star_time(&self) -> Option<u64> {
        if let Some(anchor) = &self.last_anchor {
            self.astro.get_time_from_stars(anchor)
        } else {
            None
        }
    }
    
    /// Получение позиции из звёзд (без GPS)
    pub fn get_star_position(&self) -> Option<(f64, f64)> {
        if let Some(anchor) = &self.last_anchor {
            self.astro.get_position_from_stars(anchor)
        } else {
            None
        }
    }
    
    /// Проверка необходимости выхода из гибернации
    pub fn should_wake_from_hibernation(&self, threshold_hours: u64) -> bool {
        if let Some(anchor) = &self.last_anchor {
            let now = self.astro.current_timestamp();
            let age_hours = (now - anchor.timestamp) / 3600;
            age_hours >= threshold_hours
        } else {
            true
        }
    }
}

impl Default for AstroAnchor {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for AstroPoE {
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
    fn test_star_catalog() {
        assert!(NAVIGATION_STARS.len() >= MIN_STARS_FOR_ANCHOR);
        for star in NAVIGATION_STARS {
            assert!(star.magnitude <= STELLAR_MAGNITUDE_LIMIT);
        }
    }
    
    #[test]
    fn test_anchor_creation() {
        let mut astro = AstroAnchor::new();
        let keypair = generate_keypair();
        
        // Симулируем изображение
        let image_data = vec![0u8; 1024];
        
        let result = astro.create_anchor(&image_data, 55.7558, 37.6176, &keypair);
        assert!(result.is_ok());
        
        let anchor = result.unwrap();
        assert_eq!(anchor.stars.len(), MIN_STARS_FOR_ANCHOR);
        assert!(anchor.signature != [0u8; 64]);
    }
    
    #[test]
    fn test_anchor_verification() {
        let mut astro = AstroAnchor::new();
        let keypair = generate_keypair();
        
        let image_data = vec![0u8; 1024];
        let anchor = astro.create_anchor(&image_data, 55.7558, 37.6176, &keypair).unwrap();
        
        let valid = astro.verify_anchor(&anchor);
        assert!(valid);
    }
    
    #[test]
    fn test_star_time() {
        let mut astro = AstroAnchor::new();
        let keypair = generate_keypair();
        
        let image_data = vec![0u8; 1024];
        let anchor = astro.create_anchor(&image_data, 55.7558, 37.6176, &keypair).unwrap();
        
        let star_time = astro.get_time_from_stars(&anchor);
        assert!(star_time.is_some());
        
        let now = astro.current_timestamp();
        let diff = now.saturating_sub(star_time.unwrap());
        assert!(diff < 3600); // Погрешность меньше часа
    }
    
    #[test]
    fn test_astro_poe() {
        let mut astro_poe = AstroPoE::new();
        let keypair = generate_keypair();
        
        let image_data = vec![0u8; 1024];
        let verified = astro_poe.verify_with_stars(&image_data, &keypair);
        assert!(verified);
        
        let star_time = astro_poe.get_star_time();
        assert!(star_time.is_some());
        
        let star_position = astro_poe.get_star_position();
        assert!(star_position.is_some());
    }
}

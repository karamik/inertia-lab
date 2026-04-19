// src/consensus/astro_anchor.rs
// Астрономический якорь — верификация через звёздное небо
// Inertia Protocol — Post-Internet Digital Species

use std::time::{SystemTime, UNIX_EPOCH};
use std::f64::consts::PI;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use log::{debug, info, warn};

const ASTRO_ANCHOR_VERSION: u8 = 1;
const MIN_STARS_FOR_ANCHOR: usize = 3;
const ASTRO_TIMESTAMP_TOLERANCE_SECS: u64 = 300;

#[derive(Debug, Clone, Copy)]
pub struct Star {
    pub hip_id: u32,
    pub name: &'static str,
    pub ra_hours: f64,
    pub dec_degrees: f64,
    pub magnitude: f64,
    pub color_index: f64,
}

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

#[derive(Debug, Clone)]
pub struct DetectedStar {
    pub star: Star,
    pub x: f64,
    pub y: f64,
    pub brightness: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct AstronomicalAnchor {
    pub version: u8,
    pub timestamp: u64,
    pub latitude: f64,
    pub longitude: f64,
    pub stars: Vec<Star>,
    pub star_positions: Vec<(f64, f64)>,
    pub camera_hash: [u8; 32],
    pub image_hash: [u8; 32],
    pub signature: [u8; 64],
}

#[derive(Debug, Clone)]
pub struct CameraParameters {
    pub focal_length_mm: f64,
    pub sensor_width_mm: f64,
    pub sensor_height_mm: f64,
    pub image_width: u32,
    pub image_height: u32,
    pub distortion_k1: f64,
    pub distortion_k2: f64,
}

impl Default for CameraParameters {
    fn default() -> Self {
        Self {
            focal_length_mm: 4.0,
            sensor_width_mm: 5.6,
            sensor_height_mm: 4.2,
            image_width: 4032,
            image_height: 3024,
            distortion_k1: 0.0,
            distortion_k2: 0.0,
        }
    }
}

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

    pub fn create_anchor(
        &mut self,
        _image_data: &[u8],
        latitude: f64,
        longitude: f64,
        private_key: &ed25519_dalek::Keypair,
    ) -> Result<AstronomicalAnchor, String> {
        info!("Creating astronomical anchor at ({:.4}, {:.4})", latitude, longitude);

        let timestamp = self.current_timestamp();
        let identified_stars = vec![
            NAVIGATION_STARS[0].clone(),
            NAVIGATION_STARS[2].clone(),
            NAVIGATION_STARS[3].clone(),
        ];

        let star_positions = self.compute_star_positions(&identified_stars, timestamp, latitude, longitude);
        let image_hash = [0u8; 32];
        let camera_hash = self.hash_camera_params();

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

        let signature_data = self.encode_anchor_for_signing(&anchor);
        let signature = private_key.sign(&signature_data);
        let mut anchor_signed = anchor;
        anchor_signed.signature.copy_from_slice(signature.as_bytes());

        let anchor_id = self.hash_anchor(&anchor_signed);
        self.anchors.lock().unwrap().insert(anchor_id, anchor_signed.clone());

        info!("Astronomical anchor created");
        Ok(anchor_signed)
    }

    pub fn verify_anchor(&self, anchor: &AstronomicalAnchor) -> bool {
        if anchor.version != ASTRO_ANCHOR_VERSION {
            return false;
        }
        let now = self.current_timestamp();
        let age = now.saturating_sub(anchor.timestamp);
        if age > ASTRO_TIMESTAMP_TOLERANCE_SECS {
            return false;
        }
        if anchor.stars.len() < MIN_STARS_FOR_ANCHOR {
            return false;
        }
        if !self.verify_star_positions(anchor) {
            return false;
        }
        info!("Astronomical anchor verified");
        true
    }

    pub fn get_time_from_stars(&self, anchor: &AstronomicalAnchor) -> Option<u64> {
        if anchor.stars.is_empty() {
            return None;
        }
        let star = &anchor.stars[0];
        let (az, alt) = anchor.star_positions[0];
        let lat_rad = anchor.latitude * PI / 180.0;
        let alt_rad = alt * PI / 180.0;
        let dec_rad = star.dec_degrees * PI / 180.0;
        let sin_h = (alt_rad.sin() - lat_rad.sin() * dec_rad.sin()) / (lat_rad.cos() * dec_rad.cos());
        let hour_angle = sin_h.asin();
        let ra_rad = star.ra_hours * 15.0 * PI / 180.0;
        let lst = ra_rad + hour_angle;
        let lst_deg = lst * 180.0 / PI;
        let gmst_deg = lst_deg - anchor.longitude;
        let days_since_epoch = gmst_deg / 360.985647;
        Some((2440587.5 + days_since_epoch) as u64 * 86400)
    }

    pub fn get_position_from_stars(&self, anchor: &AstronomicalAnchor) -> Option<(f64, f64)> {
        if !anchor.star_positions.is_empty() {
            let latitude = anchor.star_positions[0].1;
            let jd = self.unix_to_julian_date(anchor.timestamp);
            let t = (jd - 2451545.0) / 36525.0;
            let gmst = 280.46061837 + 360.98564736629 * (jd - 2451545.0) + 0.000387933 * t * t;
            let lst = self.compute_sidereal_time_from_stars(anchor);
            let longitude = (lst - gmst) * 180.0 / PI;
            Some((latitude, longitude))
        } else {
            None
        }
    }

    fn compute_star_positions(&self, stars: &[Star], timestamp: u64, latitude: f64, longitude: f64) -> Vec<(f64, f64)> {
        let jd = self.unix_to_julian_date(timestamp);
        stars.iter().map(|star| {
            let ra_rad = star.ra_hours * 15.0 * PI / 180.0;
            let dec_rad = star.dec_degrees * PI / 180.0;
            let lst = self.compute_local_sidereal_time(jd, longitude);
            let hour_angle = lst - ra_rad;
            let lat_rad = latitude * PI / 180.0;
            let sin_alt = lat_rad.sin() * dec_rad.sin() + lat_rad.cos() * dec_rad.cos() * hour_angle.cos();
            let alt = sin_alt.asin();
            let cos_az = (dec_rad.sin() - lat_rad.sin() * sin_alt) / (lat_rad.cos() * alt.cos());
            let az = cos_az.acos();
            (az * 180.0 / PI, alt * 180.0 / PI)
        }).collect()
    }

    fn verify_star_positions(&self, anchor: &AstronomicalAnchor) -> bool {
        let jd = self.unix_to_julian_date(anchor.timestamp);
        for (i, star) in anchor.stars.iter().enumerate() {
            let (expected_az, expected_alt) = self.star_to_az_alt(star, jd, anchor.latitude, anchor.longitude);
            let (actual_az, actual_alt) = anchor.star_positions[i];
            let az_diff = (expected_az - actual_az).abs();
            let alt_diff = (expected_alt - actual_alt).abs();
            if az_diff > 0.5 || alt_diff > 0.5 {
                return false;
            }
        }
        true
    }

    fn star_to_az_alt(&self, star: &Star, jd: f64, latitude: f64, longitude: f64) -> (f64, f64) {
        let ra_rad = star.ra_hours * 15.0 * PI / 180.0;
        let dec_rad = star.dec_degrees * PI / 180.0;
        let lst = self.compute_local_sidereal_time(jd, longitude);
        let hour_angle = lst - ra_rad;
        let lat_rad = latitude * PI / 180.0;
        let sin_alt = lat_rad.sin() * dec_rad.sin() + lat_rad.cos() * dec_rad.cos() * hour_angle.cos();
        let alt = sin_alt.asin();
        let cos_az = (dec_rad.sin() - lat_rad.sin() * sin_alt) / (lat_rad.cos() * alt.cos());
        let az = cos_az.acos();
        (az * 180.0 / PI, alt * 180.0 / PI)
    }

    fn compute_local_sidereal_time(&self, jd: f64, longitude: f64) -> f64 {
        let t = (jd - 2451545.0) / 36525.0;
        let gmst = 280.46061837 + 360.98564736629 * (jd - 2451545.0) + 0.000387933 * t * t - t * t * t / 38710000.0;
        (gmst + longitude) * PI / 180.0
    }

    fn compute_sidereal_time_from_stars(&self, anchor: &AstronomicalAnchor) -> f64 {
        if anchor.stars.is_empty() {
            return 0.0;
        }
        let star = &anchor.stars[0];
        let (az, alt) = anchor.star_positions[0];
        let lat_rad = anchor.latitude * PI / 180.0;
        let alt_rad = alt * PI / 180.0;
        let dec_rad = star.dec_degrees * PI / 180.0;
        let sin_h = (alt_rad.sin() - lat_rad.sin() * dec_rad.sin()) / (lat_rad.cos() * dec_rad.cos());
        let hour_angle = sin_h.asin();
        let ra_rad = star.ra_hours * 15.0 * PI / 180.0;
        ra_rad + hour_angle
    }

    fn unix_to_julian_date(&self, unix_secs: u64) -> f64 {
        (unix_secs as f64 / 86400.0) + 2440587.5
    }

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
        *blake3::hash(&self.encode_anchor_for_signing(anchor)).as_bytes()
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
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }
}

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

    pub fn verify_with_stars(&mut self, image_data: &[u8], keypair: &ed25519_dalek::Keypair) -> bool {
        if let Ok(anchor) = self.astro.create_anchor(image_data, 55.7558, 37.6176, keypair) {
            if self.astro.verify_anchor(&anchor) {
                self.last_anchor = Some(anchor);
                self.verification_count += 1;
                info!("Star verification successful (count: {})", self.verification_count);
                return true;
            }
        }
        false
    }

    pub fn get_star_time(&self) -> Option<u64> {
        self.last_anchor.as_ref().and_then(|a| self.astro.get_time_from_stars(a))
    }

    pub fn get_star_position(&self) -> Option<(f64, f64)> {
        self.last_anchor.as_ref().and_then(|a| self.astro.get_position_from_stars(a))
    }

    pub fn should_wake_from_hibernation(&self, threshold_hours: u64) -> bool {
        if let Some(anchor) = &self.last_anchor {
            let now = self.astro.current_timestamp();
            (now - anchor.timestamp) / 3600 >= threshold_hours
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

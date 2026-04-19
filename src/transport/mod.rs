// src/transport/mod.rs

pub mod wifi_stego;
pub mod bluetooth_adv;

#[cfg(feature = "audio-support")]
pub mod ultrasound;

#[cfg(feature = "dns-client")]
pub mod dns_spore;

pub mod lora;      // <-- ДОБАВИТЬ ЭТУ СТРОКУ
pub mod usb_transfer;

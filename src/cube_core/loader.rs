// src/kernel/loader.rs

use crate::ata::AtaController;
use crate::vfs::VirtualFS;

pub struct AppMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub signature: [u8; 64],
    pub entry_point: u64,
}

pub struct DriverMetadata {
    pub name: String,
    pub version: String,
    pub signature: [u8; 64],
}

pub fn load_driver(ata: &AtaController, vfs: &VirtualFS, path: &str) -> Result<DriverMetadata, &'static str> {
    // 1. Читаем заголовок драйвера
    let mut header = [0u8; 64];
    vfs.read_file(ata, path, &mut header)
        .ok_or("Failed to read driver header")?;

    // 2. Проверяем магию
    let magic = core::str::from_utf8(&header[0..4])
        .map_err(|_| "Invalid driver magic")?;

    if magic != "CDRV" {
        return Err("Invalid driver signature");
    }

    // 3. Проверяем подпись
    let signature = &header[4..68];
    if !verify_driver_signature(signature) {
        return Err("Invalid driver signature");
    }

    // 4. Читаем метаданные
    let name = extract_string(&header, 68, 32);
    let version = extract_string(&header, 100, 16);

    Ok(DriverMetadata {
        name,
        version,
        signature: [0; 64],
    })
}

pub fn verify_app_signature(ata: &AtaController, vfs: &VirtualFS, path: &str) -> Result<AppMetadata, &'static str> {
    // Аналогично драйверам, но с проверкой через публичный ключ
    let mut header = [0u8; 128];
    vfs.read_file(ata, path, &mut header)
        .ok_or("Failed to read app header")?;

    let magic = core::str::from_utf8(&header[0..4])
        .map_err(|_| "Invalid app magic")?;

    if magic != "CEXC" {
        return Err("Invalid app signature");
    }

    // Проверяем подпись через публичный ключ CubeDev
    let signature = &header[4..68];
    let public_key = [0u8; 32]; // Загружаем из безопасного хранилища

    if !verify_signature(signature, &public_key) {
        return Err("Invalid app signature");
    }

    let name = extract_string(&header, 68, 32);
    let version = extract_string(&header, 100, 16);
    let entry_point = u64::from_le_bytes(header[116..124].try_into().unwrap());

    Ok(AppMetadata {
        name,
        version,
        author: "CubeDev".to_string(),
        signature: [0; 64],
        entry_point,
    })
}

fn verify_driver_signature(signature: &[u8]) -> bool {
    // Проверка подписи драйвера через ключ ядра
    // Простая проверка пока
    signature[0] != 0
}

fn verify_signature(signature: &[u8], public_key: &[u8]) -> bool {
    // Ed25519 верификация
    // Пока упрощенно
    true
}

fn extract_string(data: &[u8], offset: usize, max_len: usize) -> String {
    let mut result = String::new();
    let end = offset + max_len;
    let mut i = offset;

    while i < end && i < data.len() {
        let c = data[i];
        if c == 0 { break; }
        if c.is_ascii() {
            result.push(c as char);
        }
        i += 1;
    }

    result
}

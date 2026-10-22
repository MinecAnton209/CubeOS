// src/fs/mod.rs
pub mod fat32;

use crate::ata::AtaDrive;
use crate::serial;
use fat32::Fat32;

pub struct FSManager {
    pub fat32: Option<Fat32>,
    current_dir_cluster: u32,
}

impl FSManager {
    pub const fn new() -> Self {
        Self {
            fat32: None,
            current_dir_cluster: 0,
        }
    }

    /// Монтирует FAT32 раздел по индексу (0 = первый, 1 = второй и т.д.)
    pub fn mount_fat32(&mut self, ata: &AtaDrive, partition_index: usize) -> bool {
        match Fat32::mount(ata, partition_index) {
            Some(fat) => {
                self.current_dir_cluster = fat.bpb.root_cluster;
                self.fat32 = Some(fat);
                true
            }
            None => false,
        }
    }

    /// Список файлов в текущей директории (для команды dir)
    pub fn dir(&self, ata: &AtaDrive) -> bool {
        if let Some(ref fat) = self.fat32 {
            let mut entries = [fat32::FatDirEntry::new(); 64];
            let count = fat.list_dir_entries(ata, self.current_dir_cluster, &mut entries);
            for i in 0..count {
                let entry = &entries[i];
                let mut name_buf = [0u8; 13];
                let len = entry.name_to_buf(&mut name_buf);
                if let Ok(name) = core::str::from_utf8(&name_buf[..len]) {
                    serial::send_string("  ");
                    serial::send_string(name);
                    if entry.is_directory() {
                        serial::send_string("/\n");
                    } else {
                        serial::send_string("\n");
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// Сменить текущую директорию
    pub fn change_dir(&mut self, ata: &AtaDrive, path: &str) -> bool {
        if let Some(ref fat) = self.fat32 {
            if path == ".." {
                // TODO: реализовать возврат в родительскую директорию через запись ".."
                return false;
            }
            let name83 = match Fat32::name_to_83(path) {
                Some(n) => n,
                None => return false,
            };
            let mut entries = [fat32::FatDirEntry::new(); 64];
            let count = fat.list_dir_entries(ata, self.current_dir_cluster, &mut entries);
            for i in 0..count {
                if entries[i].name == name83 && entries[i].is_directory() {
                    self.current_dir_cluster = entries[i].cluster();
                    return true;
                }
            }
        }
        false
    }

    /// Читает файл по полному пути (например, "/system/config/boot.cfg")
    pub fn read_file(&self, ata: &AtaDrive, path: &str, buf: &mut [u8]) -> Option<usize> {
        let fat = self.fat32.as_ref()?;

        // Отладка: выводим путь
        crate::serial::send_string("[FS] Reading file: ");
        crate::serial::send_string(path);
        crate::serial::send_string("\n");

        let (dir_cluster, name83) = self.resolve_path(fat, ata, path)?;

        // Отладка: выводим имя в формате 8.3
        crate::serial::send_string("[FS] Looking for: ");
        for b in &name83 {
            if *b != b' ' {
                crate::serial::send_byte(*b);
            }
        }
        crate::serial::send_string("\n");

        let mut entries = [fat32::FatDirEntry::new(); 64];
        let count = fat.list_dir_entries(ata, dir_cluster, &mut entries);

        // Отладка: выводим все записи в директории
        crate::serial::send_string("[FS] Directory entries: ");
        let mut count_buf = [0u8; 10];
        let mut i = 10;
        let mut n = count;
        if n == 0 { count_buf[9] = b'0'; i = 9; }
        else { while n > 0 { i -= 1; count_buf[i] = (n % 10) as u8 + b'0'; n /= 10; } }
        crate::serial::send_string(core::str::from_utf8(&count_buf[i..]).unwrap_or("?"));
        crate::serial::send_string("\n");

        for j in 0..count {
            crate::serial::send_string("  [");
            let mut idx_buf = [0u8; 10];
            let mut i = 10;
            let mut n = j;
            if n == 0 { idx_buf[9] = b'0'; i = 9; }
            else { while n > 0 { i -= 1; idx_buf[i] = (n % 10) as u8 + b'0'; n /= 10; } }
            crate::serial::send_string(core::str::from_utf8(&idx_buf[i..]).unwrap_or("?"));
            crate::serial::send_string("] ");
            for b in &entries[j].name {
                if *b != b' ' {
                    crate::serial::send_byte(*b);
                }
            }
            crate::serial::send_string("\n");
        }

        for j in 0..count {
            if entries[j].name == name83 && !entries[j].is_directory() {
                crate::serial::send_string("[FS] File found! Reading...\n");
                return fat.read_file(ata, entries[j].cluster(), buf, entries[j].size);
            }
        }

        crate::serial::send_string("[FS] File not found in directory\n");
        None
    }

    /// Записывает данные в файл по пути
    pub fn write_file(&self, ata: &AtaDrive, path: &str, data: &[u8]) -> bool {
        let fat = match &self.fat32 {
            Some(f) => f,
            None => return false,
        };
        let (dir_cluster, name83) = match self.resolve_path(fat, ata, path) {
            Some(r) => r,
            None => return false,
        };

        let mut entries = [fat32::FatDirEntry::new(); 64];
        let count = fat.list_dir_entries(ata, dir_cluster, &mut entries);
        for i in 0..count {
            if entries[i].name == name83 && !entries[i].is_directory() {
                let new_cluster = match fat.write_file_data(ata, entries[i].cluster(), data) {
                    Some(c) => c,
                    None => return false,
                };
                return self.update_file_size(ata, dir_cluster, &name83, data.len() as u32, new_cluster);
            }
        }

        let start_cluster = match fat.create_file(ata, dir_cluster, &name83) {
            Some(c) => c,
            None => return false,
        };
        if fat.write_file_data(ata, start_cluster, data).is_none() {
            return false;
        }
        self.update_file_size(ata, dir_cluster, &name83, data.len() as u32, start_cluster)
    }

    fn update_file_size(&self, ata: &AtaDrive, dir_cluster: u32, name83: &[u8; 11], size: u32, new_cluster: u32) -> bool {
        let fat = match &self.fat32 {
            Some(f) => f,
            None => return false,
        };
        let cluster_bytes = fat.bpb.sectors_per_cluster as usize * 512;
        let mut dir_buf = [0u8; 32768];
        let mut c = dir_cluster;
        loop {
            if !fat.read_cluster(ata, c, &mut dir_buf) {
                return false;
            }
            let entries_per_cluster = cluster_bytes / 32;
            for j in 0..entries_per_cluster {
                let off = j * 32;
                if dir_buf[off] == 0x00 {
                    return false;
                }
                if dir_buf[off..off + 11] == *name83 {
                    dir_buf[off + 20] = (new_cluster >> 16) as u8;
                    dir_buf[off + 21] = (new_cluster >> 24) as u8;
                    dir_buf[off + 26] = new_cluster as u8;
                    dir_buf[off + 27] = (new_cluster >> 8) as u8;
                    dir_buf[off + 28] = size as u8;
                    dir_buf[off + 29] = (size >> 8) as u8;
                    dir_buf[off + 30] = (size >> 16) as u8;
                    dir_buf[off + 31] = (size >> 24) as u8;
                    return fat.write_cluster(ata, c, &dir_buf);
                }
            }
            match fat.next_cluster(ata, c) {
                Some(next) => c = next,
                None => return false,
            }
        }
    }

    pub fn create_file(&self, ata: &AtaDrive, name: &str) -> bool {
        let fat = match &self.fat32 {
            Some(f) => f,
            None => return false,
        };
        let name83 = match Fat32::name_to_83(name) {
            Some(n) => n,
            None => return false,
        };
        fat.create_file(ata, self.current_dir_cluster, &name83).is_some()
    }

    pub fn create_dir(&self, ata: &AtaDrive, name: &str) -> bool {
        let fat = match &self.fat32 {
            Some(f) => f,
            None => return false,
        };
        let name83 = match Fat32::name_to_83(name) {
            Some(n) => n,
            None => return false,
        };
        fat.create_dir(ata, self.current_dir_cluster, &name83).is_some()
    }

    pub fn delete_file(&self, ata: &AtaDrive, name: &str) -> bool {
        let fat = match &self.fat32 {
            Some(f) => f,
            None => return false,
        };
        let name83 = match Fat32::name_to_83(name) {
            Some(n) => n,
            None => return false,
        };
        fat.delete_file(ata, self.current_dir_cluster, &name83)
    }

    /// Разрешает путь вида "/system/config/boot.cfg" до (dir_cluster, name83)
    fn resolve_path(&self, fat: &Fat32, ata: &AtaDrive, path: &str) -> Option<(u32, [u8; 11])> {
        if !path.starts_with('/') { return None; }

        let mut parts = ["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""];
        let mut part_count = 0;
        for part in path[1..].split('/') {
            if !part.is_empty() && part_count < 16 {
                parts[part_count] = part;
                part_count += 1;
            }
        }

        if part_count == 0 { return None; }

        let file_name = parts[part_count - 1];
        let name83 = Fat32::name_to_83(file_name)?;

        // Отладка: выводим полное имя файла в 8.3 формате (с пробелами!)
        crate::serial::send_string("[FS] Looking for file: ");
        for b in &name83 {
            crate::serial::send_byte(*b);  // Убрали проверку на пробел!
        }
        crate::serial::send_string("\n");

        let mut current = fat.bpb.root_cluster;

        for i in 0..part_count - 1 {
            let part = parts[i];
            if part.is_empty() { continue; }
            let dir_name83 = Fat32::name_to_83(part)?;

            // Отладка: выводим имя директории в 8.3 формате
            crate::serial::send_string("[FS] Looking for dir: ");
            for b in &dir_name83 {
                crate::serial::send_byte(*b);
            }
            crate::serial::send_string("\n");

            // Отладка: выводим ВСЕ записи в текущей директории
            crate::serial::send_string("[FS] Listing directory cluster ");
            let mut buf = [0u8; 20];
            let mut n = current as usize;
            let mut idx = 20;
            while n > 0 { idx -= 1; buf[idx] = (n % 10) as u8 + b'0'; n /= 10; }
            crate::serial::send_string(core::str::from_utf8(&buf[idx..]).unwrap_or("?"));
            crate::serial::send_string(":\n");

            let mut entries = [fat32::FatDirEntry::new(); 64];
            let count = fat.list_dir_entries(ata, current, &mut entries);

            for j in 0..count {
                crate::serial::send_string("  [");
                let mut idx_buf = [0u8; 10];
                let mut idx = 10;
                let mut n = j;
                if n == 0 { idx_buf[9] = b'0'; idx = 9; }
                else { while n > 0 { idx -= 1; idx_buf[idx] = (n % 10) as u8 + b'0'; n /= 10; } }
                crate::serial::send_string(core::str::from_utf8(&idx_buf[idx..]).unwrap_or("?"));
                crate::serial::send_string("] ");

                // Выводим имя записи ПОЛНОСТЬЮ (с пробелами!)
                for b in &entries[j].name {
                    crate::serial::send_byte(*b);
                }

                if entries[j].is_directory() {
                    crate::serial::send_string(" [DIR]");
                }
                crate::serial::send_string("\n");
            }

            let mut found = false;
            for j in 0..count {
                if entries[j].name == dir_name83 && entries[j].is_directory() {
                    current = entries[j].cluster();
                    found = true;
                    break;
                }
            }
            if !found {
                crate::serial::send_string("[FS] Directory not found!\n");
                return None;
            }
        }

        Some((current, name83))
    }
}

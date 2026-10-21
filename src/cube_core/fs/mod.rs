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
        let (dir_cluster, name83) = self.resolve_path(fat, ata, path)?;

        let mut entries = [fat32::FatDirEntry::new(); 64];
        let count = fat.list_dir_entries(ata, dir_cluster, &mut entries);
        for i in 0..count {
            if entries[i].name == name83 && !entries[i].is_directory() {
                return fat.read_file(ata, entries[i].cluster(), buf, entries[i].size);
            }
        }
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
        if !path.starts_with('/') {
            return None;
        }

        let mut parts = ["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""];
        let mut part_count = 0;
        for part in path[1..].split('/') {
            if !part.is_empty() && part_count < 16 {
                parts[part_count] = part;
                part_count += 1;
            }
        }
        if parts.is_empty() {
            return None;
        }

        let file_name = parts.last()?;
        let name83 = Fat32::name_to_83(file_name)?;

        let mut current = fat.bpb.root_cluster;
        for part in &parts[..parts.len() - 1] {
            if part.is_empty() {
                continue;
            }
            let dir_name83 = Fat32::name_to_83(part)?;
            let mut entries = [fat32::FatDirEntry::new(); 64];
            let count = fat.list_dir_entries(ata, current, &mut entries);
            let mut found = false;
            for i in 0..count {
                if entries[i].name == dir_name83 && entries[i].is_directory() {
                    current = entries[i].cluster();
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
        }

        Some((current, name83))
    }
}

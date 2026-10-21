// src/kernel_api/module_manager.rs

use core::slice;

pub struct ModuleManager {
    loaded_drivers: [DriverEntry; 32],
    loaded_fs: [FsEntry; 8],
    driver_count: usize,
    fs_count: usize,
}

struct DriverEntry {
    name: [u8; 32],
    entry_point: *const u8,
    initialized: bool,
}

struct FsEntry {
    name: [u8; 16],
    mount_fn: *const u8,
    mounted: bool,
}

impl ModuleManager {
    pub fn new() -> Self {
        Self {
            loaded_drivers: [DriverEntry { name: [0; 32], entry_point: core::ptr::null(), initialized: false }; 32],
            loaded_fs: [FsEntry { name: [0; 16], mount_fn: core::ptr::null(), mounted: false }; 8],
            driver_count: 0,
            fs_count: 0,
        }
    }

    pub fn load_driver(&mut self, name: &str) -> Result<(), &'static str> {
        // Ищем драйвер в /System/drivers/
        // Загружаем его как бинарный модуль
        // Регистрируем в ядре

        // Проверяем, не загружен ли уже
        for i in 0..self.driver_count {
            let entry = &self.loaded_drivers[i];
            let entry_name = core::str::from_utf8(&entry.name).unwrap_or("");
            if entry_name == name {
                return Err("Driver already loaded");
            }
        }

        if self.driver_count >= 32 {
            return Err("Too many drivers");
        }

        // Загружаем драйвер
        // В реальности здесь будет чтение с диска и загрузка в память

        let mut entry = &mut self.loaded_drivers[self.driver_count];
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(31);
        entry.name[..len].copy_from_slice(&name_bytes[..len]);
        entry.name[len] = 0;
        entry.entry_point = core::ptr::null();
        entry.initialized = false;

        self.driver_count += 1;
        Ok(())
    }

    pub fn load_filesystem(&mut self, name: &str) -> Result<(), &'static str> {
        // Аналогично драйверам
        if self.fs_count >= 8 {
            return Err("Too many filesystems");
        }

        let mut entry = &mut self.loaded_fs[self.fs_count];
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(15);
        entry.name[..len].copy_from_slice(&name_bytes[..len]);
        entry.name[len] = 0;
        entry.mount_fn = core::ptr::null();
        entry.mounted = false;

        self.fs_count += 1;
        Ok(())
    }

    pub fn mount_root(&self) -> Result<(), &'static str> {
        // Монтируем корневую ФС
        // Проверяем все загруженные ФС
        for i in 0..self.fs_count {
            let entry = &self.loaded_fs[i];
            let name = core::str::from_utf8(&entry.name).unwrap_or("");

            // Пробуем смонтировать
            if name == "fat32" {
                // Монтируем FAT32
                return Ok(());
            }
        }

        Err("No suitable filesystem found")
    }

    pub fn start_init(&self) -> Result<(), &'static str> {
        // Запускаем init процесс
        // Загружаем /System/init.exc
        Ok(())
    }
}

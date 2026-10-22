// src/fs/fat32.rs — полный FAT32 драйвер
use crate::ata::AtaDrive;

const SECTOR_SIZE: usize = 512;
const DIR_ENTRY_SIZE: usize = 32;
const FAT_ENTRY_END: u32 = 0x0FFFFFFF;
const FAT_ENTRY_FREE: u32 = 0x00000000;
const ATTR_READ_ONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

/// Запись директории FAT (8.3 формат)
#[derive(Debug, Clone, Copy)]
pub struct FatDirEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub cluster_hi: u16,
    pub cluster_lo: u16,
    pub size: u32,
}

impl FatDirEntry {
    pub const fn new() -> Self {
        Self {
            name: [0; 11],
            attr: 0,
            cluster_hi: 0,
            cluster_lo: 0,
            size: 0,
        }
    }

    pub fn is_directory(&self) -> bool {
        self.attr & ATTR_DIRECTORY != 0
    }

    pub fn is_free(&self) -> bool {
        self.name[0] == 0x00 || self.name[0] == 0xE5
    }

    pub fn is_lfn(&self) -> bool {
        self.attr == ATTR_LONG_NAME
    }

    pub fn cluster(&self) -> u32 {
        (self.cluster_hi as u32) << 16 | self.cluster_lo as u32
    }

    /// Преобразует имя 8.3 в читаемую строку с нуль-терминатором
    pub fn name_to_buf(&self, buf: &mut [u8; 13]) -> usize {
        let mut len = 0;
        // Имя (8 символов)
        for i in 0..8 {
            if self.name[i] == b' ' { break; }
            buf[len] = self.name[i];
            len += 1;
        }
        // Расширение
        let has_ext = self.name[8..11].iter().any(|&c| c != b' ');
        if has_ext {
            buf[len] = b'.';
            len += 1;
            for i in 8..11 {
                if self.name[i] == b' ' { break; }
                buf[len] = self.name[i];
                len += 1;
            }
        }
        buf[len] = 0;
        len
    }
}

/// BIOS Parameter Block FAT32
#[derive(Debug, Clone, Copy)]
pub struct BpbFat32 {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_cluster: u32,
    pub fs_info_sector: u16,
    pub backup_boot_sector: u16,
    pub fat_start_lba: u32,
    pub data_start_lba: u32,
    pub fat_size_sectors: u32,
    pub total_sectors: u32,
}

/// FAT32 том
pub struct Fat32 {
    pub bpb: BpbFat32,
    pub partition_start: u32,
}

impl Fat32 {
    /// Монтирует FAT32 раздел по индексу (0, 1, 2, 3)
    pub fn mount(ata: &AtaDrive, partition_index: usize) -> Option<Self> {
        // Читаем MBR
        let mut mbr = [0u8; 512];
        if !ata.read_sector(0, &mut mbr) {
            return None;
        }
        if mbr[510] != 0x55 || mbr[511] != 0xAA {
            return None;
        }

        // Находим нужный раздел
        let offset = 446 + partition_index * 16;
        if offset + 16 > 512 {
            return None;
        }
        let status = mbr[offset];
        let _chc = &mbr[offset + 1..offset + 4]; // CHS начала (не используем)
        let part_type = mbr[offset + 4];
        let start_lba = u32::from_le_bytes([mbr[offset + 8], mbr[offset + 9], mbr[offset + 10], mbr[offset + 11]]);
        let _size_lba = u32::from_le_bytes([mbr[offset + 12], mbr[offset + 13], mbr[offset + 14], mbr[offset + 15]]);

        // Проверяем тип раздела (0x0B = FAT32 LBA, 0x0C = FAT32 LBA extended)
        if part_type != 0x0B && part_type != 0x0C {
            return None;
        }
        if start_lba == 0 {
            return None;
        }

        // Читаем загрузочный сектор раздела
        let mut boot = [0u8; 512];
        if !ata.read_sector(start_lba, &mut boot) {
            return None;
        }
        if boot[510] != 0x55 || boot[511] != 0xAA {
            return None;
        }

        // Парсим BPB
        let bytes_per_sector = u16::from_le_bytes([boot[11], boot[12]]);
        let sectors_per_cluster = boot[13];
        let reserved_sectors = u16::from_le_bytes([boot[14], boot[15]]);
        let num_fats = boot[16];
        let root_cluster = u32::from_le_bytes([boot[44], boot[45], boot[46], boot[47]]);
        let fs_info_sector = u16::from_le_bytes([boot[48], boot[49]]);
        let backup_boot_sector = u16::from_le_bytes([boot[50], boot[51]]);
        let fat_size_sectors = u32::from_le_bytes([boot[36], boot[37], boot[38], boot[39]]);
        let total_sectors = u32::from_le_bytes([boot[32], boot[33], boot[34], boot[35]]);

        if bytes_per_sector != 512 || sectors_per_cluster == 0 || num_fats == 0 {
            return None;
        }

        let fat_start_lba = start_lba + reserved_sectors as u32;
        let data_start_lba = fat_start_lba + fat_size_sectors * num_fats as u32;

        crate::serial::send_string("[FS] BPB: bytes_per_sector=");
        let mut buf = [0u8; 20];
        let mut n = bytes_per_sector as usize;
        let mut i = 20;
        while n > 0 { i -= 1; buf[i] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[i..]).unwrap_or("?"));

        crate::serial::send_string(", sectors_per_cluster=");
        n = sectors_per_cluster as usize;
        i = 20;
        while n > 0 { i -= 1; buf[i] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[i..]).unwrap_or("?"));

        crate::serial::send_string(", root_cluster=");
        n = root_cluster as usize;
        i = 20;
        while n > 0 { i -= 1; buf[i] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[i..]).unwrap_or("?"));

        crate::serial::send_string(", fat_start_lba=");
        n = fat_start_lba as usize;
        i = 20;
        while n > 0 { i -= 1; buf[i] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[i..]).unwrap_or("?"));

        crate::serial::send_string(", data_start_lba=");
        n = data_start_lba as usize;
        i = 20;
        while n > 0 { i -= 1; buf[i] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[i..]).unwrap_or("?"));

        crate::serial::send_string("\n");

        Some(Self {
            bpb: BpbFat32 {
                bytes_per_sector,
                sectors_per_cluster,
                reserved_sectors,
                num_fats,
                root_cluster,
                fs_info_sector,
                backup_boot_sector,
                fat_start_lba: reserved_sectors as u32,  // ← убрали partition_start
                data_start_lba: reserved_sectors as u32 + fat_size_sectors * num_fats as u32,  // ← убрали partition_start
                fat_size_sectors,
                total_sectors,
            },
            partition_start: start_lba,
        })
    }

    /// Читает сектор с учётом смещения раздела
    fn read_sector(&self, ata: &AtaDrive, lba: u32, buf: &mut [u8; 512]) -> bool {
        ata.read_sector(self.partition_start + lba, buf)
    }

    /// Записывает сектор с учётом смещения раздела
    fn write_sector(&self, ata: &AtaDrive, lba: u32, buf: &[u8; 512]) -> bool {
        ata.write_sector(self.partition_start + lba, buf)
    }

    /// LBA первого сектора кластера
    pub fn cluster_to_lba(&self, cluster: u32) -> u32 {
        self.bpb.data_start_lba + (cluster - 2) * self.bpb.sectors_per_cluster as u32
    }

    /// Читает весь кластер в буфер (должен быть >= sectors_per_cluster * 512)
    pub fn read_cluster(&self, ata: &AtaDrive, cluster: u32, buf: &mut [u8]) -> bool {
        let lba = self.cluster_to_lba(cluster);
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        if buf.len() < cluster_bytes {
            return false;
        }
        for i in 0..self.bpb.sectors_per_cluster {
            let mut sector = [0u8; 512];
            if !self.read_sector(ata, lba + i as u32, &mut sector) {
                return false;
            }
            let off = i as usize * SECTOR_SIZE;
            buf[off..off + SECTOR_SIZE].copy_from_slice(&sector);
        }
        true
    }

    /// Записывает кластер из буфера
    pub fn write_cluster(&self, ata: &AtaDrive, cluster: u32, buf: &[u8]) -> bool {
        let lba = self.cluster_to_lba(cluster);
        for i in 0..self.bpb.sectors_per_cluster {
            let off = i as usize * SECTOR_SIZE;
            let mut sector = [0u8; 512];
            sector.copy_from_slice(&buf[off..off + SECTOR_SIZE]);
            if !self.write_sector(ata, lba + i as u32, &sector) {
                return false;
            }
        }
        true
    }

    /// Читает FAT entry для кластера
    pub fn read_fat_entry(&self, ata: &AtaDrive, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = self.bpb.fat_start_lba + fat_offset / SECTOR_SIZE as u32;
        let fat_offset_in_sector = (fat_offset % SECTOR_SIZE as u32) as usize;

        let mut sector = [0u8; 512];
        if !self.read_sector(ata, fat_sector, &mut sector) {
            return None;
        }

        let entry = u32::from_le_bytes([
            sector[fat_offset_in_sector],
            sector[fat_offset_in_sector + 1],
            sector[fat_offset_in_sector + 2],
            sector[fat_offset_in_sector + 3],
        ]) & 0x0FFFFFFF;

        Some(entry)
    }

    /// Записывает FAT entry (обновляет обе копии FAT)
    pub fn write_fat_entry(&self, ata: &AtaDrive, cluster: u32, value: u32) -> bool {
        let fat_offset = cluster * 4;
        let fat_sector = self.bpb.fat_start_lba + fat_offset / SECTOR_SIZE as u32;
        let fat_offset_in_sector = (fat_offset % SECTOR_SIZE as u32) as usize;

        for fat_copy in 0..self.bpb.num_fats {
            let sector_lba = fat_sector + fat_copy as u32 * self.bpb.fat_size_sectors;
            let mut sector = [0u8; 512];
            if !self.read_sector(ata, sector_lba, &mut sector) {
                return false;
            }
            sector[fat_offset_in_sector] = (value & 0xFF) as u8;
            sector[fat_offset_in_sector + 1] = ((value >> 8) & 0xFF) as u8;
            sector[fat_offset_in_sector + 2] = ((value >> 16) & 0xFF) as u8;
            sector[fat_offset_in_sector + 3] = ((value >> 24) & 0x0F) as u8;
            if !self.write_sector(ata, sector_lba, &sector) {
                return false;
            }
        }
        true
    }

    /// Получает следующий кластер в цепочке
    pub fn next_cluster(&self, ata: &AtaDrive, cluster: u32) -> Option<u32> {
        let entry = self.read_fat_entry(ata, cluster)?;
        if entry >= 0x0FFFFFF8 {
            None // Конец цепочки
        } else {
            Some(entry)
        }
    }

    /// Устанавливает следующий кластер
    pub fn set_next_cluster(&self, ata: &AtaDrive, cluster: u32, next: u32) -> bool {
        self.write_fat_entry(ata, cluster, next)
    }

    /// Находит первый свободный кластер
    pub fn find_free_cluster(&self, ata: &AtaDrive) -> Option<u32> {
        // Начинаем с кластера 3 (0 и 1 зарезервированы, 2 = root)
        for cluster in 3..(self.bpb.total_sectors / self.bpb.sectors_per_cluster as u32) {
            if let Some(entry) = self.read_fat_entry(ata, cluster) {
                if entry == FAT_ENTRY_FREE {
                    return Some(cluster);
                }
            }
        }
        None
    }

    /// Читает файл по стартовому кластеру в буфер. Возвращает количество прочитанных байт.
    pub fn read_file(&self, ata: &AtaDrive, start_cluster: u32, buf: &mut [u8], file_size: u32) -> Option<usize> {
        let mut cluster = start_cluster;
        let mut total_read = 0usize;
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let mut cluster_buf = [0u8; 32768]; // до 64 секторов на кластер

        while total_read < buf.len() && total_read < file_size as usize {
            if !self.read_cluster(ata, cluster, &mut cluster_buf) {
                return Some(total_read);
            }
            let to_copy = cluster_bytes.min(buf.len() - total_read).min(file_size as usize - total_read);
            buf[total_read..total_read + to_copy].copy_from_slice(&cluster_buf[..to_copy]);
            total_read += to_copy;

            match self.next_cluster(ata, cluster) {
                Some(next) => cluster = next,
                None => break,
            }
        }
        Some(total_read)
    }

    /// Записывает данные в файл, начиная со start_cluster.
    /// Если данных больше, чем помещается в существующую цепочку, выделяет новые кластеры.
    /// Возвращает новый стартовый кластер (может измениться, если файл был пустой).
    pub fn write_file_data(&self, ata: &AtaDrive, start_cluster: u32, data: &[u8]) -> Option<u32> {
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let mut cluster = if start_cluster == 0 {
            // Файл пустой — выделяем первый кластер
            let c = self.find_free_cluster(ata)?;
            if !self.write_fat_entry(ata, c, FAT_ENTRY_END) { return None; };
            c
        } else {
            start_cluster
        };

        let mut offset = 0usize;
        let mut current = cluster;

        while offset < data.len() {
            let mut cluster_buf = [0u8; 32768];
            // Если это не первый кластер и у текущего есть следующий — переходим
            if offset > 0 {
                match self.next_cluster(ata, current) {
                    Some(next) => current = next,
                    None => {
                        // Нужно выделить новый кластер
                        let next = self.find_free_cluster(ata)?;
                        if !self.write_fat_entry(ata, current, next) { return None; };
                        if !self.write_fat_entry(ata, next, FAT_ENTRY_END) { return None; };
                        current = next;
                    }
                }
            }

            let to_write = cluster_bytes.min(data.len() - offset);
            cluster_buf[..to_write].copy_from_slice(&data[offset..offset + to_write]);
            // Остаток кластера заполняем нулями
            for b in cluster_buf[to_write..cluster_bytes].iter_mut() {
                *b = 0;
            }

            if !self.write_cluster(ata, current, &cluster_buf) {
                return None;
            }
            offset += to_write;
        }

        // Если файл стал короче — обрезаем цепочку
        let needed_clusters = (data.len() + cluster_bytes - 1) / cluster_bytes;
        let mut c = cluster;
        for _ in 1..needed_clusters {
            if let Some(next) = self.next_cluster(ata, c) {
                c = next;
            }
        }
        // Обрезаем после последнего нужного кластера
        if let Some(extra) = self.next_cluster(ata, c) {
            if !self.write_fat_entry(ata, c, FAT_ENTRY_END) { return None; };
            // Освобождаем остаток цепочки
            let mut free_c = extra;
            loop {
                match self.next_cluster(ata, free_c) {
                    Some(next) => {
                        if !self.write_fat_entry(ata, free_c, FAT_ENTRY_FREE) { return None; };
                        free_c = next;
                    }
                    None => {
                        if !self.write_fat_entry(ata, free_c, FAT_ENTRY_FREE) { return None; };
                        break;
                    }
                }
            }
        }

        Some(cluster)
    }

    /// Листинг директории. Возвращает количество записей.
    pub fn list_dir_entries(&self, ata: &AtaDrive, dir_cluster: u32, entries: &mut [FatDirEntry]) -> usize {
        let mut count = 0;
        let mut cluster = dir_cluster;
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let entries_per_cluster = cluster_bytes / DIR_ENTRY_SIZE;
        let mut cluster_buf = [0u8; 32768];

        loop {
            if !self.read_cluster(ata, cluster, &mut cluster_buf) {
                break;
            }

            for i in 0..entries_per_cluster {
                if count >= entries.len() {
                    return count;
                }
                let off = i * DIR_ENTRY_SIZE;
                let first_byte = cluster_buf[off];
                if first_byte == 0x00 {
                    return count; // Конец директории
                }
                if first_byte == 0xE5 {
                    continue; // Удалённая запись
                }

                let attr = cluster_buf[off + 11];
                if attr == ATTR_LONG_NAME {
                    continue; // Пропускаем LFN
                }

                let mut entry = FatDirEntry::new();
                entry.name.copy_from_slice(&cluster_buf[off..off + 11]);
                entry.attr = attr;
                entry.cluster_lo = u16::from_le_bytes([cluster_buf[off + 26], cluster_buf[off + 27]]);
                entry.cluster_hi = u16::from_le_bytes([cluster_buf[off + 20], cluster_buf[off + 21]]);
                entry.size = u32::from_le_bytes([
                    cluster_buf[off + 28],
                    cluster_buf[off + 29],
                    cluster_buf[off + 30],
                    cluster_buf[off + 31],
                ]);
                entries[count] = entry;
                count += 1;
            }

            match self.next_cluster(ata, cluster) {
                Some(next) => cluster = next,
                None => break,
            }
        }
        count
    }

    /// Находит запись в директории по имени (8.3, без учёта регистра)
    pub fn find_entry(&self, ata: &AtaDrive, dir_cluster: u32, name83: &[u8; 11]) -> Option<(u32, usize)> {
        let mut cluster = dir_cluster;
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let entries_per_cluster = cluster_bytes / DIR_ENTRY_SIZE;
        let mut cluster_buf = [0u8; 32768];
        let mut global_index = 0usize;

        loop {
            if !self.read_cluster(ata, cluster, &mut cluster_buf) {
                return None;
            }

            for i in 0..entries_per_cluster {
                let off = i * DIR_ENTRY_SIZE;
                let first_byte = cluster_buf[off];
                if first_byte == 0x00 {
                    return None;
                }
                if first_byte == 0xE5 {
                    global_index += 1;
                    continue;
                }
                let attr = cluster_buf[off + 11];
                if attr == ATTR_LONG_NAME {
                    global_index += 1;
                    continue;
                }

                let entry_name = &cluster_buf[off..off + 11];
                if entry_name == name83 {
                    return Some((cluster, global_index));
                }
                global_index += 1;
            }

            match self.next_cluster(ata, cluster) {
                Some(next) => cluster = next,
                None => return None,
            }
        }
    }

    /// Находит свободную запись в директории (или создаёт новый кластер)
    pub fn find_free_dir_entry(&self, ata: &AtaDrive, dir_cluster: u32) -> Option<(u32, usize)> {
        let mut cluster = dir_cluster;
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let entries_per_cluster = cluster_bytes / DIR_ENTRY_SIZE;
        let mut cluster_buf = [0u8; 32768];

        loop {
            if !self.read_cluster(ata, cluster, &mut cluster_buf) {
                return None;
            }

            for i in 0..entries_per_cluster {
                let off = i * DIR_ENTRY_SIZE;
                let first_byte = cluster_buf[off];
                if first_byte == 0x00 || first_byte == 0xE5 {
                    return Some((cluster, i));
                }
            }

            match self.next_cluster(ata, cluster) {
                Some(next) => cluster = next,
                None => {
                    // Директория заполнена — выделяем новый кластер
                    let new_cluster = self.find_free_cluster(ata)?;
                    if !self.write_fat_entry(ata, new_cluster, FAT_ENTRY_END) { return None; };
                    if !self.write_fat_entry(ata, cluster, new_cluster) { return None; };
                    // Очищаем новый кластер
                    let empty = [0u8; 32768];
                    if !self.write_cluster(ata, new_cluster, &empty) { return None; };
                    return Some((new_cluster, 0));
                }
            }
        }
    }

    /// Преобразует короткое имя в формат 8.3 (11 байт, верхний регистр, пробелы)
    pub fn name_to_83(name: &str) -> Option<[u8; 11]> {
        let mut result = [b' '; 11];
        let mut upper = [0u8; 64];
        let mut upper_len = 0;

        for b in name.bytes() {
            if upper_len < 64 {
                upper[upper_len] = b.to_ascii_uppercase();
                upper_len += 1;
            }
        }
        let upper = &upper[..upper_len];

        let (name_part, ext_part) = if let Some(dot_pos) = upper.iter().position(|&b| b == b'.') {
            (&upper[..dot_pos], &upper[dot_pos + 1..])
        } else {
            (&upper[..], &[][..])
        };

        if name_part.len() > 8 || ext_part.len() > 3 {
            return None;
        }

        for (i, &b) in name_part.iter().enumerate() {
            result[i] = b;
        }
        for (i, &b) in ext_part.iter().enumerate() {
            result[8 + i] = b;
        }

        Some(result)
    }

    /// Создаёт файл в директории. Возвращает стартовый кластер.
    pub fn create_file(&self, ata: &AtaDrive, dir_cluster: u32, name83: &[u8; 11]) -> Option<u32> {
        // Проверяем, что файла ещё нет
        if self.find_entry(ata, dir_cluster, name83).is_some() {
            return None;
        }

        let (cluster, entry_idx) = self.find_free_dir_entry(ata, dir_cluster)?;
        let new_cluster = self.find_free_cluster(ata)?;
        if !self.write_fat_entry(ata, new_cluster, FAT_ENTRY_END) { return None; };

        // Читаем кластер директории и добавляем запись
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let mut dir_buf = [0u8; 32768];
        if !self.read_cluster(ata, cluster, &mut dir_buf) {
            return None;
        }

        let off = entry_idx * DIR_ENTRY_SIZE;
        dir_buf[off..off + 11].copy_from_slice(name83);
        dir_buf[off + 11] = ATTR_ARCHIVE; // атрибуты
        dir_buf[off + 12] = 0; // NT reserved
        dir_buf[off + 13] = 0; // creation time fine
        // Время/дата создания — нули (можно заполнить)
        dir_buf[off + 20] = (new_cluster >> 16) as u8;
        dir_buf[off + 21] = (new_cluster >> 24) as u8;
        dir_buf[off + 26] = new_cluster as u8;
        dir_buf[off + 27] = (new_cluster >> 8) as u8;
        dir_buf[off + 28] = 0; // размер = 0
        dir_buf[off + 29] = 0;
        dir_buf[off + 30] = 0;
        dir_buf[off + 31] = 0;

        if !self.write_cluster(ata, cluster, &dir_buf) {
            return None;
        }

        Some(new_cluster)
    }

    /// Создаёт директорию. Возвращает стартовый кластер.
    pub fn create_dir(&self, ata: &AtaDrive, dir_cluster: u32, name83: &[u8; 11]) -> Option<u32> {
        if self.find_entry(ata, dir_cluster, name83).is_some() {
            return None;
        }

        let (cluster, entry_idx) = self.find_free_dir_entry(ata, dir_cluster)?;
        let new_cluster = self.find_free_cluster(ata)?;
        if !self.write_fat_entry(ata, new_cluster, FAT_ENTRY_END) { return None; };

        // Очищаем новый кластер
        let empty = [0u8; 32768];
        if !self.write_cluster(ata, new_cluster, &empty) { return None; };

        // Создаём записи "." и ".."
        let mut dir_buf = [0u8; 32768];
        // "."
        dir_buf[0..11].copy_from_slice(b".          ");
        dir_buf[11] = ATTR_DIRECTORY;
        dir_buf[20] = (new_cluster >> 16) as u8;
        dir_buf[21] = (new_cluster >> 24) as u8;
        dir_buf[26] = new_cluster as u8;
        dir_buf[27] = (new_cluster >> 8) as u8;
        // ".."
        dir_buf[32..43].copy_from_slice(b"..         ");
        dir_buf[43] = ATTR_DIRECTORY;
        dir_buf[52] = (dir_cluster >> 16) as u8;
        dir_buf[53] = (dir_cluster >> 24) as u8;
        dir_buf[58] = dir_cluster as u8;
        dir_buf[59] = (dir_cluster >> 8) as u8;

        if !self.write_cluster(ata, new_cluster, &dir_buf) { return None; };

        // Добавляем запись в родительскую директорию
        let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
        let mut parent_buf = [0u8; 32768];
        if !self.read_cluster(ata, cluster, &mut parent_buf) {
            return None;
        }
        let off = entry_idx * DIR_ENTRY_SIZE;
        parent_buf[off..off + 11].copy_from_slice(name83);
        parent_buf[off + 11] = ATTR_DIRECTORY;
        parent_buf[off + 20] = (new_cluster >> 16) as u8;
        parent_buf[off + 21] = (new_cluster >> 24) as u8;
        parent_buf[off + 26] = new_cluster as u8;
        parent_buf[off + 27] = (new_cluster >> 8) as u8;
        parent_buf[off + 28] = 0;
        parent_buf[off + 29] = 0;
        parent_buf[off + 30] = 0;
        parent_buf[off + 31] = 0;
        if !self.write_cluster(ata, cluster, &parent_buf) { return None; };

        Some(new_cluster)
    }

    /// Удаляет файл по имени в директории
    pub fn delete_file(&self, ata: &AtaDrive, dir_cluster: u32, name83: &[u8; 11]) -> bool {
        if let Some((cluster, entry_idx)) = self.find_entry(ata, dir_cluster, name83) {
            let cluster_bytes = self.bpb.sectors_per_cluster as usize * SECTOR_SIZE;
            let mut dir_buf = [0u8; 32768];
            if !self.read_cluster(ata, cluster, &mut dir_buf) {
                return false;
            }
            let off = entry_idx * DIR_ENTRY_SIZE;
            let file_cluster_lo = u16::from_le_bytes([dir_buf[off + 26], dir_buf[off + 27]]);
            let file_cluster_hi = u16::from_le_bytes([dir_buf[off + 20], dir_buf[off + 21]]);
            let file_cluster = (file_cluster_hi as u32) << 16 | file_cluster_lo as u32;

            // Помечаем запись как удалённую
            dir_buf[off] = 0xE5;
            if !self.write_cluster(ata, cluster, &dir_buf) {
                return false;
            }

            // Освобождаем цепочку кластеров
            if file_cluster >= 2 {
                let mut c = file_cluster;
                loop {
                    match self.next_cluster(ata, c) {
                        Some(next) => {
                            if !self.write_fat_entry(ata, c, FAT_ENTRY_FREE) { return false; };
                            c = next;
                        }
                        None => {
                            if !self.write_fat_entry(ata, c, FAT_ENTRY_FREE) { return false; };
                            break;
                        }
                    }
                }
            }
            true
        } else {
            false
        }
    }
}

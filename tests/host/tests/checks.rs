use checks_host::checks::{
    cluster_in_range, config_cmdline, elf_segment_fits, fat_layout, in_segment, mmap_entry_count, range_fits,
};

#[test]
fn range_fits_rejects_wraparound() {
    assert!(!range_fits(u64::MAX - 1, 4, u64::MAX));
    assert!(range_fits(10, 5, 15));
    assert!(!range_fits(10, 6, 15));
}

#[test]
fn in_segment_bounds_are_half_open() {
    assert!(in_segment(100, 100, 10));
    assert!(in_segment(109, 100, 10));
    assert!(!in_segment(110, 100, 10));
    assert!(!in_segment(99, 100, 10));
}

#[test]
fn elf_segment_rejects_filesz_above_memsz() {
    assert!(!elf_segment_fits(0x100000, 0, 8, 4, 64, 0x100000, 0x200000));
}

#[test]
fn elf_segment_rejects_file_offset_overflow() {
    assert!(!elf_segment_fits(0x100000, u64::MAX - 1, 4, 4, 64, 0x100000, 0x200000));
}

#[test]
fn elf_segment_rejects_addresses_below_region_or_past_its_end() {
    assert!(!elf_segment_fits(0x1000, 0, 4, 4, 64, 0x100000, 0x200000));
    assert!(!elf_segment_fits(0x1FFFFC, 0, 4, 16, 64, 0x100000, 0x200000));
}

#[test]
fn elf_segment_rejects_vaddr_wraparound() {
    assert!(!elf_segment_fits(u64::MAX - 10, 0, 4, 100, 64, 0, u64::MAX));
}

#[test]
fn elf_segment_accepts_well_formed_segment() {
    assert!(elf_segment_fits(0x300000, 0, 64, 128, 64, 0x100000, 0x200000 + 0x200000));
}

#[test]
fn cluster_in_range_excludes_reserved_and_out_of_volume_clusters() {
    assert!(!cluster_in_range(0, 100));
    assert!(!cluster_in_range(1, 100));
    assert!(cluster_in_range(2, 100));
    assert!(cluster_in_range(101, 100));
    assert!(!cluster_in_range(102, 100));
}

#[test]
fn fat_layout_accepts_valid_geometry() {
    let layout = fat_layout(1000, 32, 2, 100, 8, 2).expect("valid geometry");
    assert_eq!(layout.data_start_rel, 232);
    assert_eq!(layout.cluster_count, 96);
}

#[test]
fn fat_layout_rejects_bad_sectors_per_cluster() {
    assert!(fat_layout(1000, 32, 2, 100, 0, 2).is_none());
    assert!(fat_layout(1000, 32, 2, 100, 3, 2).is_none());
    assert!(fat_layout(1000, 32, 2, 100, 128, 2).is_none());
}

#[test]
fn fat_layout_rejects_zero_reserved_fats_or_fat_size() {
    assert!(fat_layout(1000, 0, 2, 100, 8, 2).is_none());
    assert!(fat_layout(1000, 32, 0, 100, 8, 2).is_none());
    assert!(fat_layout(1000, 32, 2, 0, 8, 2).is_none());
}

#[test]
fn fat_layout_rejects_overflowing_fat_sizes() {
    assert!(fat_layout(1000, 32, 2, u32::MAX, 8, 2).is_none());
}

#[test]
fn fat_layout_rejects_root_cluster_outside_the_volume() {
    assert!(fat_layout(1000, 32, 2, 100, 8, 0).is_none());
    assert!(fat_layout(1000, 32, 2, 100, 8, 1).is_none());
    assert!(fat_layout(1000, 32, 2, 100, 8, 1000).is_none());
}

#[test]
fn fat_layout_rejects_more_clusters_than_fat32_can_address() {
    assert!(fat_layout(u32::MAX, 32, 1, 100, 1, 2).is_none());
}

#[test]
fn mmap_entry_count_rejects_zero_or_too_small_entry_sizes() {
    assert_eq!(mmap_entry_count(64, 16, 0, 24), 0);
    assert_eq!(mmap_entry_count(64, 16, 8, 24), 0);
}

#[test]
fn mmap_entry_count_rejects_tags_shorter_than_their_header() {
    assert_eq!(mmap_entry_count(8, 16, 24, 24), 0);
}

#[test]
fn mmap_entry_count_divides_payload_evenly() {
    assert_eq!(mmap_entry_count(16 + 48 + 5, 16, 24, 24), 2);
}

#[test]
fn config_cmdline_reads_lf_terminated_config() {
    assert_eq!(config_cmdline("# cfg\nmode=normal\ncmdline=\"/bin/csh\"\n"), Some("/bin/csh"));
}

#[test]
fn config_cmdline_handles_crlf_without_slicing_errors() {
    assert_eq!(config_cmdline("# cfg\r\nmode=normal\r\ncmdline=\"/bin/csh\"\r\n"), Some("/bin/csh"));
}

#[test]
fn config_cmdline_accepts_last_line_without_newline() {
    assert_eq!(config_cmdline("cmdline = /bin/x"), Some("/bin/x"));
}

#[test]
fn config_cmdline_ignores_empty_values_and_missing_keys() {
    assert_eq!(config_cmdline("cmdline=\"\"\n"), None);
    assert_eq!(config_cmdline("mode=normal\n"), None);
    assert_eq!(config_cmdline(""), None);
}

#[test]
fn config_cmdline_last_non_empty_assignment_wins() {
    assert_eq!(config_cmdline("cmdline=/bin/a\ncmdline=/bin/b\n"), Some("/bin/b"));
}

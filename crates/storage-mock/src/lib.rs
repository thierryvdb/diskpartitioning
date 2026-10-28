//! Fixtures 100% sintéticas (AGENTS.md: nenhum acesso a discos reais).

use disk_domain::*;

pub struct MockProvider;

const fn gib(n: u64, sector: u64) -> u64 {
    n * 1024 * 1024 * 1024 / sector
}

#[allow(clippy::too_many_arguments)]
fn part(
    id: &str,
    name: &str,
    start: u64,
    len: u64,
    fs: Fs,
    kind: Kind,
    used: Option<u64>,
) -> Partition {
    Partition {
        id: id.into(),
        name: name.into(),
        start_lba: start,
        length_sectors: len,
        fs,
        kind,
        used_sectors: used,
        r#virtual: false,
    }
}

impl StorageProvider for MockProvider {
    fn discover(&self) -> Vec<Disk> {
        let g = |n| gib(n, 512);
        let g4 = |n| gib(n, 4096);
        vec![
            Disk {
                id: "disk0".into(),
                model: "Mock NVMe 512 GB".into(),
                bus: "NVMe".into(),
                table: Table::Gpt,
                sector_size: 512,
                total_sectors: g(512),
                first_usable_lba: 34,
                last_usable_excl_lba: g(512) - 34,
                partitions: vec![
                    part(
                        "d0p1",
                        "EFI",
                        2048,
                        204_800,
                        Fs::Fat32,
                        Kind::Efi,
                        Some(60_000),
                    ),
                    part("d0p2", "MSR", 206_848, 32_768, Fs::None, Kind::Msr, None),
                    part(
                        "d0p3",
                        "C:",
                        239_616,
                        g(300),
                        Fs::Ntfs,
                        Kind::Normal,
                        Some(g(180)),
                    ),
                    part(
                        "d0p4",
                        "Recovery",
                        239_616 + g(300),
                        1_048_576,
                        Fs::Ntfs,
                        Kind::Recovery,
                        Some(800_000),
                    ),
                ],
            },
            Disk {
                id: "disk1".into(),
                model: "Mock SATA 1 TB (Linux)".into(),
                bus: "SATA".into(),
                table: Table::Gpt,
                sector_size: 512,
                total_sectors: g(1024),
                first_usable_lba: 34,
                last_usable_excl_lba: g(1024) - 34,
                partitions: vec![
                    part(
                        "d1p1",
                        "/",
                        2048,
                        g(200),
                        Fs::Ext4,
                        Kind::Normal,
                        Some(g(90)),
                    ),
                    part(
                        "d1p2",
                        "swap",
                        2048 + g(200),
                        g(16),
                        Fs::Swap,
                        Kind::Normal,
                        None,
                    ),
                    part(
                        "d1p3",
                        "dados",
                        2048 + g(300),
                        g(400),
                        Fs::Unknown,
                        Kind::Normal,
                        None,
                    ),
                ],
            },
            Disk {
                id: "disk2".into(),
                model: "Mock USB 64 GB (4Kn)".into(),
                bus: "USB".into(),
                table: Table::Mbr,
                sector_size: 4096,
                total_sectors: g4(64),
                first_usable_lba: 1,
                last_usable_excl_lba: g4(64),
                partitions: vec![
                    part(
                        "d2p1",
                        "PEN1",
                        256,
                        g4(8),
                        Fs::Exfat,
                        Kind::Normal,
                        Some(g4(2)),
                    ),
                    part(
                        "d2p2",
                        "PEN2",
                        256 + g4(20),
                        g4(4),
                        Fs::Fat32,
                        Kind::Normal,
                        Some(g4(1)),
                    ),
                ],
            },
        ]
    }
}

//! Utilidades compartilhadas pelos adaptadores de inventário (somente leitura).
//! `run_readonly` é o ÚNICO ponto do projeto que cria processos: aceita apenas programas da allowlist,
//! com argumentos fixos definidos pelos adaptadores (nunca texto vindo da UI), com timeout.

use disk_domain::{Kind, Table};
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Programas de LEITURA permitidos. Qualquer outro é recusado.
const ALLOWED_PROGRAMS: [&str; 2] = ["powershell", "lsblk"];
const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_OUTPUT: usize = 8 << 20;

pub const GPT_EFI: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
pub const GPT_MSR: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae";
pub const GPT_RECOVERY: &str = "de94bba4-06d1-4d40-a16a-bfd50179d6ac";

pub fn kind_from_gpt(guid: &str) -> Kind {
    match guid
        .trim_matches(|c| c == '{' || c == '}')
        .to_ascii_lowercase()
        .as_str()
    {
        GPT_EFI => Kind::Efi,
        GPT_MSR => Kind::Msr,
        GPT_RECOVERY => Kind::Recovery,
        _ => Kind::Normal,
    }
}

pub fn kind_from_mbr(t: u8) -> Kind {
    match t {
        0xEF => Kind::Efi,
        0x27 => Kind::Recovery,
        _ => Kind::Normal,
    }
}

/// `[primeiro, último_exclusivo)` utilizáveis. GPT: cabeçalho + 128 entradas de 128 bytes nas duas pontas.
pub fn usable_range(table: Table, total_sectors: u64, sector_size: u64) -> Option<(u64, u64)> {
    match table {
        Table::Mbr => Some((1, total_sectors)),
        Table::Gpt => {
            let entries = 128 * 128_u64 / sector_size;
            let (first, tail) = (2 + entries, 1 + entries);
            (total_sectors > first + tail).then(|| (first, total_sectors - tail))
        }
    }
}

/// Executa um programa de leitura da allowlist e devolve o stdout (UTF-8, sem BOM).
pub fn run_readonly(program: &str, args: &[&str]) -> Result<String, String> {
    if !ALLOWED_PROGRAMS.contains(&program) {
        return Err(format!("programa não permitido: {program}"));
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("falha ao iniciar {program}: {e}"))?;
    let mut out = child.stdout.take().ok_or("sem stdout")?;
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        out.by_ref()
            .take(MAX_OUTPUT as u64)
            .read_to_end(&mut buf)
            .map(|_| buf)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break s,
            None if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                return Err(format!("{program}: tempo esgotado"));
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "leitura interrompida")?
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("{program} terminou com erro ({status})"));
    }
    Ok(String::from_utf8_lossy(&bytes)
        .trim_start_matches('\u{feff}')
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recusa_programas_fora_da_allowlist() {
        for p in ["diskpart", "cmd", "sh", "mkfs", "../lsblk"] {
            assert!(
                run_readonly(p, &[]).unwrap_err().contains("não permitido"),
                "{p}"
            );
        }
    }

    #[test]
    fn tipos_gpt_e_mbr() {
        assert_eq!(
            kind_from_gpt("{C12A7328-F81F-11D2-BA4B-00A0C93EC93B}"),
            Kind::Efi
        );
        assert_eq!(kind_from_gpt(GPT_MSR), Kind::Msr);
        assert_eq!(kind_from_gpt(GPT_RECOVERY), Kind::Recovery);
        assert_eq!(
            kind_from_gpt("ebd0a0a2-b9e5-4433-87c0-68b6b72699c7"),
            Kind::Normal
        );
        assert_eq!(kind_from_mbr(0xEF), Kind::Efi);
        assert_eq!(kind_from_mbr(0x07), Kind::Normal);
    }

    #[test]
    fn faixa_utilizavel() {
        assert_eq!(usable_range(Table::Gpt, 1000, 512), Some((34, 967)));
        assert_eq!(usable_range(Table::Gpt, 1000, 4096), Some((6, 995)));
        assert_eq!(usable_range(Table::Mbr, 1000, 512), Some((1, 1000)));
        assert_eq!(usable_range(Table::Gpt, 10, 512), None);
    }
}

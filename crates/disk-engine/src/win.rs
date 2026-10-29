//! Backend Windows do `BlockDevice` (ADR 0003). Só é usado pela ferramenta de linha de comando
//! `gdp-mbr`, nunca pelo app. A identidade vem do `Get-Disk` (somente leitura) e é recusado
//! qualquer disco que não seja USB, ou que seja de boot/sistema.
use crate::{BlockDevice, Identity};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};

pub struct WinDevice {
    file: File,
    id: Identity,
}

/// Lê (somente leitura) serial/tamanho e recusa discos que não possam ser alvo.
pub fn query_identity(number: u32) -> Result<Identity, String> {
    // `number` é u32: nenhum texto externo entra no script.
    let script = format!(
        "$d=Get-Disk -Number {number}; \"$($d.SerialNumber.Trim())|$($d.Size)|$($d.BusType)|$($d.IsBoot)|$($d.IsSystem)\""
    );
    let out = storage_common::run_readonly("powershell", &["-NoProfile", "-NonInteractive", "-Command", &script])?;
    let f: Vec<&str> = out.trim().split('|').collect();
    if f.len() != 5 {
        return Err(format!("saída inesperada do Get-Disk: {out:?}"));
    }
    if f[2] != "USB" {
        return Err(format!("disco {number} não é USB ({})", f[2]));
    }
    if f[3] != "False" || f[4] != "False" {
        return Err(format!("disco {number} é de boot/sistema"));
    }
    Ok(Identity { serial: f[0].to_string(), size_bytes: f[1].parse().map_err(|_| "tamanho inválido")? })
}

impl WinDevice {
    /// Abre `\\.\PhysicalDriveN`. Exige terminal elevado; este código não pede elevação.
    pub fn open(number: u32, write: bool) -> Result<Self, String> {
        let id = query_identity(number)?;
        let file = OpenOptions::new()
            .read(true)
            .write(write)
            .open(format!(r"\\.\PhysicalDrive{number}"))
            .map_err(|e| format!("não foi possível abrir o disco {number}: {e}"))?;
        // Reconfere depois de abrir: o número do disco pode ter mudado no meio do caminho.
        if query_identity(number)? != id {
            return Err("a identidade do disco mudou durante a abertura".into());
        }
        Ok(Self { file, id })
    }
}

impl BlockDevice for WinDevice {
    fn identity(&self) -> &Identity {
        &self.id
    }
    fn read_at(&self, off: u64, buf: &mut [u8]) -> io::Result<()> {
        let mut f = &self.file;
        f.seek(SeekFrom::Start(off))?;
        f.read_exact(buf)
    }
    fn write_at(&mut self, off: u64, data: &[u8]) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(off))?;
        self.file.write_all(data)?;
        self.file.sync_all()
    }
}

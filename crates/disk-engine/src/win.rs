//! Backend Windows do `BlockDevice` (ADR 0003). Só é usado pela ferramenta de linha de comando
//! `gdp-mbr`, nunca pelo app. A identidade vem do `Get-Disk` (somente leitura) e é recusado
//! qualquer disco que não seja USB, ou que seja de boot/sistema.
pub use crate::Fs;
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
    let out = storage_common::run_readonly(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )?;
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
    Ok(Identity {
        serial: f[0].to_string(),
        size_bytes: f[1].parse().map_err(|_| "tamanho inválido")?,
    })
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

#[derive(Debug, PartialEq, Eq)]
pub struct PartInfo {
    pub offset: u64,
    pub size: u64,
    pub fs: String,
}

/// Somente leitura. `disk`/`part` são u32: nenhum texto externo entra no script.
pub fn partition_info(disk: u32, part: u32) -> Result<PartInfo, String> {
    let script = format!(
        "$p=Get-Partition -DiskNumber {disk} -PartitionNumber {part} -ErrorAction Stop; \
         $v=$p | Get-Volume -ErrorAction SilentlyContinue; \"$($p.Offset)|$($p.Size)|$($v.FileSystem)\""
    );
    let out = storage_common::run_readonly(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )?;
    let f: Vec<&str> = out.trim().split('|').collect();
    if f.len() != 3 {
        return Err(format!("saída inesperada: {out:?}"));
    }
    Ok(PartInfo {
        offset: f[0].parse().map_err(|_| "offset inválido")?,
        size: f[1].parse().map_err(|_| "tamanho inválido")?,
        fs: f[2].to_string(),
    })
}

/// ÚNICO ponto de formatação. Reconfere a identidade do disco (USB, não boot/sistema, serial e tamanho)
/// imediatamente antes e verifica o sistema de arquivos resultante depois.
pub fn format_partition(disk: u32, target: &Identity, part: u32, fs: Fs) -> Result<(), String> {
    if query_identity(disk)? != *target {
        return Err("o disco atual não é o alvo aprovado".into());
    }
    partition_info(disk, part)?; // a partição precisa existir neste disco
    let script = format!(
        "Get-Partition -DiskNumber {disk} -PartitionNumber {part} -ErrorAction Stop | \
         Format-Volume -FileSystem {} -Confirm:$false -ErrorAction Stop | Out-Null",
        fs.name()
    );
    let st = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;
    if !st.status.success() {
        return Err(format!(
            "Format-Volume falhou: {}",
            String::from_utf8_lossy(&st.stderr)
        ));
    }
    let after = partition_info(disk, part)?;
    if !after.fs.eq_ignore_ascii_case(fs.name()) {
        return Err(format!(
            "verificação falhou: sistema de arquivos é {:?}",
            after.fs
        ));
    }
    Ok(())
}

/// Hash da aprovação: amarra disco, serial, tamanho, partição e sistema de arquivos.
pub fn format_hash(disk: u32, target: &Identity, part: u32, fs: Fs) -> u64 {
    crate::plan_hash_str(&format!(
        "format disk={disk} serial={} size={} part={part} fs={}",
        target.serial,
        target.size_bytes,
        fs.name()
    ))
}

/// Prévia do que `apply` fará. Exige terminal/app elevado (abre o disco só para leitura).
#[derive(Debug)]
pub struct Preview {
    pub disk_number: u32,
    pub identity: Identity,
    pub steps: Vec<String>,
    pub hash: u64,
    translated: crate::apply::Translated,
}

fn disk_number(id: &str) -> Result<u32, String> {
    id.strip_prefix("disk")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("id de disco inesperado: {id}"))
}

pub fn preview(disk: &disk_domain::Disk, ops: &[disk_domain::Op]) -> Result<Preview, String> {
    let n = disk_number(&disk.id)?;
    let dev = WinDevice::open(n, false)?;
    let table = crate::read_table(&dev).map_err(|e| format!("{e:?}"))?;
    let translated = crate::apply::translate(disk, &table, ops)?;
    let identity = dev.identity().clone();
    let hash = crate::plan_hash_str(&format!(
        "apply disk={n} serial={} size={} {:?} {:?}",
        identity.serial, identity.size_bytes, translated.mbr, translated.formats
    ));
    Ok(Preview {
        disk_number: n,
        identity,
        steps: translated.summary.clone(),
        hash,
        translated,
    })
}

/// Executa o plano: exige o serial digitado e o hash da prévia (recalculada agora).
pub fn apply(
    disk: &disk_domain::Disk,
    ops: &[disk_domain::Op],
    typed_serial: &str,
    hash: u64,
) -> Result<Vec<String>, String> {
    let pv = preview(disk, ops)?;
    if typed_serial != pv.identity.serial {
        return Err("o serial digitado não confere com o disco".into());
    }
    if hash != pv.hash {
        return Err("o plano ou o disco mudou desde a prévia; gere a prévia de novo".into());
    }
    let (n, t) = (pv.disk_number, &pv.translated);
    let mut log = vec![];
    if !t.mbr.is_empty() {
        let mut dev = WinDevice::open(n, true)?;
        let ap = crate::Approval {
            serial: pv.identity.serial.clone(),
            size_bytes: pv.identity.size_bytes,
            plan_hash: crate::plan_hash(&t.mbr),
        };
        crate::execute(&mut dev, &pv.identity, &ap, &t.mbr).map_err(|e| format!("{e:?}"))?;
        log.push("Tabela MBR gravada e verificada por releitura.".to_string());
    }
    if !t.formats.is_empty() {
        // Pede ao Windows para reler a tabela (não altera o disco) e só então localiza as partições.
        let _ = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!("Update-Disk -Number {n}"),
            ])
            .output();
    }
    for f in &t.formats {
        let q = format!(
            "(Get-Partition -DiskNumber {n} | Where-Object Offset -eq {}).PartitionNumber",
            f.start_lba * 512
        );
        let num = storage_common::run_readonly(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", &q],
        )
        .ok()
        .and_then(|o| o.trim().parse::<u32>().ok());
        match num {
            Some(p) => {
                format_partition(n, &pv.identity, p, f.fs)?;
                log.push(format!("Partição {p} formatada como {} e verificada.", f.fs.name()));
            }
            None => log.push(format!(
                "O Windows ainda não enxerga a partição no setor {}: reconecte o disco e formate de novo.",
                f.start_lba
            )),
        }
    }
    Ok(log)
}

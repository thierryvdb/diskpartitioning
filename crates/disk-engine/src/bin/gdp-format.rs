//! Ferramenta de teste da fase 0.2: formata UMA partição (FAT32 ou NTFS). NÃO faz parte do app.
//! Padrão = simulação; só formata com `--confirm <hash>`. Rode em terminal elevado:
//!   gdp-format --disk 1 --serial S --size B --partition 1 --fs FAT32 [--confirm HASH]
#[cfg(not(windows))]
fn main() {
    eprintln!("gdp-format só está implementado para Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    if let Err(e) = run() {
        eprintln!("ERRO: {e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run() -> Result<(), String> {
    use disk_engine::win::{format_hash, format_partition, partition_info, query_identity, Fs};
    use disk_engine::Identity;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.windows(2)
            .find(|w| w[0] == k)
            .map(|w| w[1].clone())
            .ok_or(format!("falta {k}"))
    };
    let num = |k: &str| get(k)?.parse::<u64>().map_err(|_| format!("{k} inválido"));
    let (disk, part) = (num("--disk")? as u32, num("--partition")? as u32);
    let target = Identity {
        serial: get("--serial")?,
        size_bytes: num("--size")?,
    };
    let fs = Fs::parse(&get("--fs")?).ok_or("--fs deve ser FAT32 ou NTFS")?;
    let confirm = get("--confirm")
        .ok()
        .map(|h| h.parse::<u64>().map_err(|_| "--confirm inválido"))
        .transpose()?;

    let id = query_identity(disk)?;
    if id != target {
        return Err(format!(
            "o disco {disk} tem serial {} e {} bytes, diferente do alvo informado",
            id.serial, id.size_bytes
        ));
    }
    let p = partition_info(disk, part)?;
    let hash = format_hash(disk, &target, part, fs);
    println!(
        "Disco {disk}: serial {} · {} bytes · USB, não é boot/sistema",
        id.serial, id.size_bytes
    );
    println!(
        "Partição {part}: início {} · {} bytes · sistema de arquivos atual: {:?}",
        p.offset, p.size, p.fs
    );
    println!(
        "Formatar como {} (APAGA o conteúdo da partição). Hash: {hash}",
        fs.name()
    );
    let Some(c) = confirm else {
        println!("SIMULAÇÃO: nada foi formatado. Para formatar, repita com --confirm {hash}");
        return Ok(());
    };
    if c != hash {
        return Err("--confirm não corresponde a este plano".into());
    }
    format_partition(disk, &target, part, fs)?;
    println!(
        "OK: partição {part} formatada como {} e verificada.",
        fs.name()
    );
    Ok(())
}

//! Ferramenta de teste da fase 0.2 (MBR: criar/excluir partição primária). NÃO é parte do app.
//! Padrão = simulação (dry-run): lê o MBR e mostra o plano. Só grava com `--confirm <hash do plano>`.
//! Uso (terminal elevado):
//!   gdp-mbr --disk 1 --serial S --size B --op "create 0 2048 100000 07" [--op "delete 1"] [--confirm HASH]
#[cfg(not(windows))]
fn main() {
    eprintln!("gdp-mbr só está implementado para Windows.");
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
    use disk_engine::{
        check, execute, plan_hash, read_table, win::WinDevice, Approval, BlockDevice, Identity, Op,
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| args.windows(2).find(|w| w[0] == k).map(|w| w[1].clone());
    let disk: u32 = get("--disk")
        .ok_or("falta --disk")?
        .parse()
        .map_err(|_| "--disk inválido")?;
    let target = Identity {
        serial: get("--serial").ok_or("falta --serial")?,
        size_bytes: get("--size")
            .ok_or("falta --size")?
            .parse()
            .map_err(|_| "--size inválido")?,
    };
    let confirm: Option<u64> = get("--confirm")
        .map(|h| h.parse().map_err(|_| "--confirm inválido"))
        .transpose()?;

    let mut ops = Vec::new();
    for w in args.windows(2).filter(|w| w[0] == "--op") {
        let p: Vec<&str> = w[1].split_whitespace().collect();
        let num = |i: usize| {
            p.get(i)
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or(format!("op inválida: {}", w[1]))
        };
        ops.push(match p.first().copied() {
            Some("create") => Op::Create {
                slot: num(1)? as usize,
                start: num(2)?,
                sectors: num(3)?,
                kind: p
                    .get(4)
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
                    .ok_or(format!("op inválida: {}", w[1]))?,
            },
            Some("delete") => Op::Delete {
                slot: num(1)? as usize,
            },
            _ => return Err(format!("op inválida: {}", w[1])),
        });
    }
    if ops.is_empty() {
        return Err("nenhuma --op".into());
    }

    let hash = plan_hash(&ops);
    let mut dev = WinDevice::open(disk, confirm.is_some())?;
    println!(
        "Disco {disk}: serial {} · {} bytes",
        dev.identity().serial,
        dev.identity().size_bytes
    );
    println!(
        "MBR atual: {:#?}",
        read_table(&dev).map_err(|e| format!("{e:?}"))?
    );
    println!("Plano: {ops:#?}\nHash do plano: {hash}");

    check(&dev, &ops).map_err(|e| format!("plano inválido para este disco: {e:?}"))?;
    println!("Plano VÁLIDO para o MBR atual.");
    let Some(c) = confirm else {
        println!("SIMULAÇÃO: nada foi gravado. Para gravar, repita com --confirm {hash}");
        return Ok(());
    };
    let approval = Approval {
        serial: target.serial.clone(),
        size_bytes: target.size_bytes,
        plan_hash: c,
    };
    execute(&mut dev, &target, &approval, &ops).map_err(|e| format!("{e:?}"))?;
    println!(
        "OK: gravado e verificado por releitura. MBR agora: {:#?}",
        read_table(&dev).map_err(|e| format!("{e:?}"))?
    );
    Ok(())
}

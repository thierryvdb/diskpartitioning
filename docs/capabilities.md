# Matriz inicial de capacidades (MVP 0.1)

Fonte da verdade: `disk_planner::capabilities` (Rust). Toda ação viável é **somente simulação**; nada grava em disco.

| Alvo | Redimensionar | Mover | Formatar | Excluir | Clonar partição |
|---|---|---|---|---|---|
| NTFS / FAT32 / exFAT / ext4 / swap (partição normal) | simulação | simulação | simulação | simulação | simulação |
| FS desconhecido (criptografado, LVM, RAID, btrfs/xfs…) | **bloqueado** | **bloqueado** | simulação | simulação | simulação |
| EFI / MSR / Recovery | **bloqueado** | **bloqueado** | **bloqueado** | **bloqueado** | simulação |

Ações de disco: **clonar disco inteiro** exige mesmo tamanho de setor e capacidade suficiente (validado no planejador); destino é sobrescrito (aviso).

Regras que a matriz espelha (o planejador as aplica de novo ao simular): partição protegida, FS desconhecido, uso mínimo no FS, sobreposição, limites do disco e máximo de 4 partições no MBR.

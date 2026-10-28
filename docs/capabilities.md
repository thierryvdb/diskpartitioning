# Matriz inicial de capacidades (MVP 0.1)

Fonte da verdade: `disk_planner::capabilities` (Rust). Toda ação viável é **somente simulação**; nada grava em disco.

| Alvo | Redimensionar | Mover | Formatar | Excluir | Clonar | Mesclar | Dividir | Rótulo | Tipo |
|---|---|---|---|---|---|---|---|---|---|
| NTFS / FAT32 / exFAT / ext4 / swap (normal) | sim. | sim. | sim. | sim. | sim. | sim. | sim. | sim.* | sim. |
| FS desconhecido (criptografado, LVM, RAID, btrfs/xfs…) | **bloq.** | **bloq.** | sim. | sim. | sim. | **bloq.** | **bloq.** | **bloq.** | sim. |
| EFI / MSR / Recovery | **bloq.** | **bloq.** | **bloq.** | **bloq.** | sim. | **bloq.** | **bloq.** | **bloq.** | **bloq.** |

"sim." = somente simulação. *Rótulo respeita o limite do FS (NTFS 32, FAT32 11 em maiúsculas, exFAT 15, ext4 16 bytes, swap 15 bytes). Tipo: só Recovery ou EFI (EFI exige FAT32); MSR não é atribuível.

Ações de disco: **clonar disco inteiro** exige mesmo tamanho de setor e capacidade suficiente (validado no planejador); destino é sobrescrito (aviso).

Regras que a matriz espelha (o planejador as aplica de novo ao simular): partição protegida, FS desconhecido, uso mínimo no FS, sobreposição, limites do disco e máximo de 4 partições no MBR.

## Ações de disco (simulação)
- **Converter GPT↔MBR:** MBR exige no máximo 4 partições, nenhuma MSR e partições dentro do limite de 32 bits (2 TiB com setor de 512); GPT exige espaço no início e no fim para a tabela. Conversão é marcada como arriscada.
- **Mesclar:** só partições adjacentes, com FS conhecido; se o uso é conhecido, os dados da segunda precisam caber no espaço livre da primeira.
- **Dividir:** a nova partição começa alinhada a 1 MiB; arquivos excedentes precisam caber nela.

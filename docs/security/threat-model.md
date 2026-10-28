# Modelo de ameaças e revisão de segurança — MVP 0.1

Escopo: app Tauri 2 (Rust + React), somente leitura/simulação, dados mock. Regras: `AGENTS.md` (REGRA MÁXIMA e §7).

## Superfície atual
| Item | Estado | Evidência |
|---|---|---|
| Escrita/elevação em dispositivos | Inexistente | Teste `no_write_paths` (`crates/disk-planner`) falha se o código de produção citar `diskpart`, `mkfs`, `parted`, `DeviceIoControl`, `Command::new`, `CreateFileW` ou `/dev/` |
| Comando genérico de shell | Inexistente | `invoke_handler`: só `list_disks`, `simulate_plan`, `export_snapshot`, `export_plan`, `import_plan` |
| Leitura de discos reais | Inexistente | Única fonte é `storage-mock`; adaptadores reais exigem autorização do proprietário |
| Permissões Tauri | Mínimas | `core:default` + `opener:allow-open-url` restrito a `https://github.com/thierryvdb/diskpartitioning*`; sem fs/shell/http |
| CSP | Restritiva | `default-src 'self'` (sem scripts remotos/inline); `style-src` permite inline por causa de estilos React |
| Exportação | Pasta de dados do app, nomes fixos | `write_export` aceita só `snapshot.json` e `plano-simulado.json`; o caminho nunca vem da UI |
| Importação | Validada | Limite de 1 MiB, schema versionado, serde tipado; snapshot com fingerprint (detecta adulteração) |
| Dados sensíveis | Nenhum | O domínio não modela serial/WWN; logs (`tracing`) registram só contagens e nomes de arquivo fixos |

## Ameaças e mitigações
- **Plano malicioso importado** → entra só como lista de `Op` tipada; simulação é pura (sem E/S); `Aplicar` não existe. Plano de layout diferente é marcado como obsoleto (`stale`).
- **Overflow/aritmética de LBA** → `checked_add` em `Partition::end`; testes de overflow no planejador.
- **TOCTOU entre inventário e uso** → fora do MVP (sem escrita); previsto no gate de mudança de fase.
- **Link externo** → somente via plugin opener, com escopo da URL do repositório oficial.
- **Dependências** → versões travadas em `Cargo.lock`/`package-lock.json`; revisar com `cargo audit`/`npm audit` antes de release (não executado: ferramentas não instaladas/CI ainda não configurado).

## Lacunas conhecidas (não resolvidas)
- Sem assinatura de código dos instaladores.
- `npm audit`: 6 avisos (2 críticos, 1 alto) só em ferramentas de desenvolvimento (vite/vitest/esbuild/tinypool); `npm audit --omit=dev` = 0. Corrigir exige atualização major (`--force`), adiada. Servidor de dev do Vite só roda localmente.
- Revisão manual, não auditoria independente.
- Gate de mudança de fase para escrita (ADR, matriz de suporte, executor isolado, autorização) ainda por definir quando o marco 0.2 for aprovado.

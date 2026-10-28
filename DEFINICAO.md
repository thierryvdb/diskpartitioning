# Definição do Produto — Gerenciador de Discos e Partições

**Versão:** 1.0 (especificação inicial)  
**Plataformas:** Windows 10/11 e Linux desktop (referência: Ubuntu 24.04 LTS)  
**Estratégia:** MVP seguro e incremental; operações avançadas após validação  
**Inspiração funcional:** PartitionMagic clássico, sem reutilização de código, marcas ou elementos proprietários.

## 1. Visão e objetivos

Construir uma aplicação desktop multiplataforma para inventariar, visualizar, planejar e, em versões posteriores, executar alterações em discos e partições. O usuário deverá compreender o layout atual e o proposto, os riscos, as limitações e o progresso de cada operação. **Integridade dos dados tem prioridade sobre conveniência ou velocidade.**

### 1.1 Princípios inegociáveis

1. O MVP 0.1 é **somente leitura** em dispositivos reais. A fila e o botão Aplicar são simulações; nenhuma escrita em disco é autorizada.
2. A interface nunca executa operações privilegiadas diretamente. Um executor isolado será introduzido em versão posterior.
3. Partição, volume, sistema de arquivos, ponto de montagem e dispositivo físico são entidades distintas.
4. Toda operação real exige pré-condições verificadas, identidade do dispositivo revalidada, confirmação explícita e estratégia documentada de recuperação.
5. **Rollback universal não existe**. Cada operação declara garantias possíveis, pontos de irreversibilidade e caminhos de recuperação.
6. Funcionalidades não implementadas, não testadas ou não suportadas não aparecem como executáveis.
7. O mesmo modelo de domínio e planejador deve operar com adaptadores Windows, Linux e Mock.

## 2. Público, escopo e cenários

**Usuários:** administradores de sistemas, técnicos de manutenção, usuários avançados e laboratórios de suporte.

**Casos principais:** examinar múltiplos discos; comparar espaço ocupado/livre/não alocado; simular criação e redimensionamento; preparar sequências de mudanças; descobrir incompatibilidades antes de aplicar; posteriormente executar tarefas suportadas em dispositivos devidamente autorizados.

**Fora do escopo do MVP:** qualquer escrita em dispositivos reais; recuperação forense; RAID/LVM/Storage Spaces como alvo de mutação; gerenciador de boot; migração de SO; operação remota; conversão arbitrária entre sistemas de arquivos; garantia de preservação de dados sem backup.

## 3. Stack tecnológica

| Camada | Escolha | Responsabilidade |
|---|---|---|
| Interface | Tauri 2, React, TypeScript, Vite | Aplicação desktop e comunicação IPC restrita |
| Visual | Tailwind CSS, shadcn/ui, SVG responsivo | Mapa proporcional dos intervalos; acessibilidade |
| Estado UI | Zustand | Seleção, filtros, fila visual; nunca fonte autoritativa do disco |
| Domínio e planejamento | Rust (crates independentes) | Tipos, invariantes, dependências, cálculos e simulação |
| Persistência | SQLite (metadados e configurações) | Snapshots exportados, preferências, planos e versões futuras do journal |
| Telemetria local | tracing | Logs estruturados, níveis e correlação, sem dados sensíveis por padrão |
| Integração Windows | Storage Management API / WMI-CIM / Win32 | Inventário; no futuro, operações oficialmente suportadas |
| Integração Linux | sysfs, udev, libblkid, util-linux; libparted | Inventário; no futuro, edição de tabela de partições |
| Testes | cargo test, proptest, Vitest, Playwright, VMs | Invariantes, integração e interface |
| CI | GitHub Actions | Build e testes em Windows/Linux, empacotamento e artefatos |

**Nota:** VDS não é a API primária para novos aplicativos Windows. No Linux, alterar limites de uma partição não equivale a redimensionar seu sistema de arquivos. Wrappers CLI, quando usados, precisam de argumentos tipados, ambiente controlado, timeout, saída interpretada por código e versão — nunca interpolação de shell.

## 4. Arquitetura e limites de confiança

```text
React UI (sem privilégios)
   -> IPC Tauri com comandos específicos e permissões mínimas
Application Services -> Disk Inventory / Operation Planner / Layout Simulator
   -> Domain (tipos e invariantes independentes do SO)
   -> StorageProvider (Windows / Linux / Mock) [somente leitura no MVP]
   -> SnapshotRepository

FUTURO: Application -> IPC autenticado -> Privileged Executor
   -> validação independente -> journal/checkpoints -> adaptador de escrita
```

**Módulos propostos:** `apps/desktop`, `crates/disk-domain`, `disk-inventory`, `disk-planner`, `storage-windows`, `storage-linux`, `storage-mock`, `storage-persistence`; depois `storage-executor`, `storage-filesystems` e `storage-recovery`.

**Segurança IPC:** expor comandos de escopo estreito (`list_disks`, `get_snapshot`, `validate_plan`, `simulate_plan`), não um `run_command` genérico. Validar entradas no backend, limitar payloads e impedir que caminhos ou IDs fornecidos pela UI sejam interpretados como autoridade para escrita. O executor privilegiado futuro deve autenticar o cliente, aplicar política de operações, limitar recursos e resistir à substituição do dispositivo (TOCTOU).

## 5. Funcionalidades do MVP 0.1

| ID | Requisito | Critério de aceite |
|---|---|---|
| MVP-01 | Detectar discos físicos, USB e virtuais visíveis ao SO | Inventário consistente com ferramentas do SO; erros parciais visíveis |
| MVP-02 | Identificar capacidade, modelo, caminho, IDs, tamanho lógico/físico do setor e estado read-only | Campos ausentes representados como desconhecidos |
| MVP-03 | Ler GPT/MBR, tipos, GUIDs, flags e intervalos das partições | Sem sobreposição falsa; intervalos válidos por setor |
| MVP-04 | Identificar volumes, letras/montagens, FS, rótulo, uso e espaço disponível | Não confundir FS livre com espaço não alocado |
| MVP-05 | Exibir mapa proporcional por disco com partições e lacunas | Escala baseada em setores, legenda, zoom e tooltip acessível |
| MVP-06 | Mostrar propriedades e avisos (EFI, recovery, boot, swap, criptografia, RAID/LVM desconhecido) | Volume não interpretável identificado, nunca presumido seguro |
| MVP-07 | Atualizar inventário e detectar alterações externas | Novo snapshot e aviso de planos obsoletos |
| MVP-08 | Criar fila **somente simulada**: criar, excluir, formatar, resize e mover | Nenhuma chamada de escrita; operações marcadas como simulação |
| MVP-09 | Desfazer/refazer, remover, reordenar operações e mostrar dependências | Replanejamento determinístico; conflitos explicados |
| MVP-10 | Prévia lado a lado: layout atual e simulado | Snapshot real permanece imutável |
| MVP-11 | Validar capacidade, alinhamento, interseção, limites GPT/MBR, setores e dependências | Erros estruturados e bloqueio de planos inválidos |
| MVP-12 | Mock provider e fixtures: GPT/MBR, 512e/4Kn, USB, espaço fragmentado, volumes desconhecidos | UI completa operável sem hardware |
| MVP-13 | Exportar diagnóstico sanitizado e plano simulado em JSON | Schema versionado; nenhum segredo obrigatório |
| MVP-14 | Interface PT-BR, tema claro/escuro, teclado e acessibilidade | Navegação sem mouse nos fluxos principais |
| MVP-15 | Tratamento de falhas, logs e testes automatizados | Falhas de uma unidade não impedem listar as demais |
| MVP-16 | Aplicar desabilitado com motivo explícito | Teste garante ausência de caminhos de escrita em produção do MVP |

### 5.1 Requisitos de UI/UX

- Painel lateral de dispositivos; cabeçalho com capacidade, tabela de partição e saúde de descoberta.
- Barra horizontal por disco com proporção real, cores por categoria e indicação independente de espaço usado/livre **dentro** do volume.
- Tabela detalhada com início/fim, tamanho, tipo, sistema de arquivos, pontos de montagem e proteção.
- Menu contextual com ações disponíveis para simulação e explicação de impedimentos.
- Painel de tarefas pendentes com origem, destino, impacto, ordem e dependências.
- Comparador antes/depois; confirmações somente para operações reais em fases futuras.
- Estados de carregamento, erro, indisponibilidade de permissões e dispositivos removidos durante leitura.
- Não depender exclusivamente de cor; disponibilizar rótulos, ícones e descrições.

## 6. Catálogo completo de funcionalidades futuras (paridade funcional inspirada no PartitionMagic)

As operações abaixo são **objetivos de produto**, não promessas universais de compatibilidade. Cada uma requer matriz de capacidades por SO/FS e testes específicos.

| Código | Funcionalidade | Condições e observações essenciais |
|---|---|---|
| ADV-01 | Criar partição primária/lógica (MBR) ou GPT | Validar entradas reservadas, alinhamento e limites MBR/GPT |
| ADV-02 | Excluir partição | Advertência de perda de dados; exclusão não equivale a limpeza segura |
| ADV-03 | Formatar NTFS, FAT32, exFAT, ext4 e FS suportados | Validar implementador e opções; formato destrutivo salvo caso particular comprovado |
| ADV-04 | Redimensionar partição (shrink/extend) | Coordenar tabela e FS na ordem correta; verificar limites reais |
| ADV-05 | Mover partição para outro offset | Motor de cópia por blocos, mapa de realocação e recuperação após interrupção |
| ADV-06 | Mesclar partições adjacentes | Migração de arquivos + resize; nunca presumir operação atômica |
| ADV-07 | Dividir partição | Shrink + criar + copiar arquivos escolhidos, incluindo metadados e ACLs |
| ADV-08 | Copiar/clonar partição | Origem/destino explícitos, preservação de atributos, IDs e riscos de duplicidade |
| ADV-09 | Copiar/clonar disco inteiro | Partições, metadados GPT/MBR, assinaturas, boot e ajuste de tamanho |
| ADV-10 | Converter FAT/FAT32 para NTFS | Apenas quando caminho não destrutivo for oficialmente suportado |
| ADV-11 | Converter GPT ↔ MBR | Potencialmente destrutivo; assistente com elegibilidade e exigências de boot |
| ADV-12 | Alterar letra/ponto de montagem | Respeitar volumes do SO, associações e montagem ativa |
| ADV-13 | Ocultar/exibir partição | Flags e semântica variam entre MBR/GPT e SO |
| ADV-14 | Alterar rótulo do volume | Respeitar limitações do FS |
| ADV-15 | Alterar tipo/flags da partição | Proteção especial para EFI, MSR, recovery e boot |
| ADV-16 | Marcar partição ativa em MBR | Não oferecer indiscriminadamente para GPT/UEFI |
| ADV-17 | Verificar consistência do sistema de arquivos | chkdsk/fsck apropriado; não executar reparo automático sem autorização |
| ADV-18 | Verificar superfície / diagnóstico de leitura | Diferenciar erros de mídia de FS; evitar testes destrutivos |
| ADV-19 | Inspecionar propriedades avançadas | Setores, offsets, UUID/GUID, atributos, alinhamento e geometria |
| ADV-20 | Explorar arquivos de volumes suportados | Acesso somente leitura por padrão; permissões e criptografia respeitadas |
| ADV-21 | Pré-visualizar antes/depois e aplicar fila | Plano validado, ordem topológica, revalidação imediatamente antes da escrita |
| ADV-22 | Desfazer operações pendentes | Desfazer simulação; distinguir de reversão de uma escrita já executada |
| ADV-23 | Recuperação de partição perdida | Produto/módulo posterior; varredura não destrutiva e validação humana |
| ADV-24 | Criar mídia offline WinPE/Live Linux | Boot seguro e operações em volumes normalmente ocupados |
| ADV-25 | Redimensionar/mover partição de SO em offline | Incluir bootloader, criptografia e recuperação de inicialização no plano |
| ADV-26 | Executar operações em sequência | Journal durável, checkpoints, progresso real e cancelamento nos pontos seguros |
| ADV-27 | Redimensionar através de espaço contíguo | Não prometer estender para espaço não contíguo sem movimento prévio |
| ADV-28 | Recuperar plano interrompido | Verificar estágio e integridade; retomar só com garantia específica |
| ADV-29 | Relatórios e histórico de operações | Preservar evidências e status verificáveis, sem alegar sucesso prematuro |
| ADV-30 | Assistentes de tarefas | Orientação contextual para expansão, migração e preparação de disco |

**Recursos considerados separadamente, mediante decisão de produto:** limpeza segura/secure erase, recuperação forense, edição hexadecimal, reconstrução de boot, RAID/LVM/Storage Spaces e gerenciador de sistemas operacionais. Não devem ser incluídos implicitamente em operações comuns.

## 7. Matriz de compatibilidade e capacidades

Não codificar suporte apenas por extensão ou nome do FS. Cada combinação deve relatar `supported`, `simulation_only`, `offline_required`, `unsupported` ou `unknown`, com justificativa, backend, versão e pré-condições.

| Sistema de arquivos / cenário | Inventário MVP | Resize futuro | Move/Merge/Split futuro |
|---|---|---|---|
| NTFS no Windows | Sim, quando acessível | Candidato via API suportada | Exige motor de movimentação/migração e testes |
| FAT32/exFAT | Sim, quando acessível | Somente após verificação do motor para SO/versão | Não presumir suporte |
| ext4 no Linux | Sim, quando acessível | Grow/shrink com regras diferentes de online/offline | Exige ferramentas de FS e motor específico |
| XFS | Identificação | Não habilitar shrink sem suporte comprovado | Exigir migração/cópia quando aplicável |
| Btrfs | Identificação | Capacidade dependente do volume e topologia | Não tratar como partição simples |
| BitLocker/LUKS bloqueado | Identificar criptografia quando possível | Bloqueado até fluxo específico validado | Bloqueado por padrão |
| LVM/RAID/Storage Spaces | Inventariar e sinalizar topologia | Fora do escopo inicial | Fora do escopo inicial |
| FS desconhecido | Identificar sem inferir conteúdo | Bloqueado | Bloqueado |

Uma operação de resize pode requerer alterar primeiro o FS e depois a partição, ou a ordem inversa, conforme expandir/reduzir e o sistema. A lógica é especializada e testada por backend, nunca genérica.

## 8. Modelo de domínio

### 8.1 Entidades obrigatórias

- `PhysicalDisk`: identidade estável quando disponível, caminhos transitórios, capacidade em bytes, setor lógico/físico, tipo de mídia, flags de remoção, read-only e estado.
- `PartitionTable`: GPT/MBR/desconhecido, cabeçalhos/entradas reservadas, GUID/assinatura, validade e limites.
- `Partition`: ID, disco, número, `start_lba`, `length_sectors`, tipo, GUID, flags, proteção; associação com volumes.
- `Volume`: ID, partições/extent(s), FS, montagem/letra, rótulo, uso, disponibilidade, criptografia e status.
- `FileSystem`: tipo, versão/capacidades observadas, estado de integridade, limites conhecidos e requisitos online/offline.
- `FreeSpaceRegion`: início e comprimento em setores, alinhamento e origem de cálculo.
- `DiskSnapshot`: captura imutável com ID, timestamp, geração, origem, versões e fingerprint de layout.
- `PendingOperation`: ID único, operação tipada, alvos, parâmetros, IDs virtuais, pré-condições, dependências e estado de planejamento.
- `OperationPlan`: snapshot-base, grafo de dependências, operações, layout previsto, avisos, bloqueios e status.
- `StorageCapability`: ação, suporte por alvo, pré-condições, modo online/offline, evidência do backend e motivo.
- `ValidationIssue`: código estável, severidade, alvo, contexto, mensagem e recomendação.

### 8.2 Invariantes

- Endereço e tamanho de armazenamento são inteiros (`u64`); multiplicações e somas são verificadas com `checked_*` ou aritmética ampliada.
- Partições são intervalos semiabertos `[start_lba, end_lba_exclusive)`; o fim é calculado e validado, sem overflow.
- Intervalos de partições não podem sobrepor outros nem regiões reservadas; MBR estendido/EBRs exigem representação própria.
- Espaço não alocado é derivado da tabela de partições, e não do espaço livre de arquivos.
- IDs são separados de nomes e caminhos mutáveis; identificadores ausentes ou duplicados desencadeiam restrições.
- Snapshot é imutável; simular produz outro layout sem efeito externo.
- Tarefas dependentes podem referenciar IDs virtuais criados pelo próprio plano.
- Um plano é invalidado quando o fingerprint ou geração do dispositivo muda.
- `OperationStatus` de planejamento é distinto de `ExecutionStatus` físico futuro.

### 8.3 Contratos iniciais (sem implementação de escrita)

```rust
trait StorageProvider {
    fn discover(&self) -> Result<DiskSnapshot, StorageError>;
    fn capabilities(&self, target: &TargetId) -> Result<Vec<StorageCapability>, StorageError>;
}

trait OperationPlanner {
    fn validate(&self, base: &DiskSnapshot, queue: &[PendingOperation])
        -> Result<ValidationReport, PlannerError>;
    fn simulate(&self, base: &DiskSnapshot, queue: &[PendingOperation])
        -> Result<OperationPlan, PlannerError>;
}

trait SnapshotRepository {
    fn save(&self, snapshot: &DiskSnapshot) -> Result<(), RepositoryError>;
    fn load(&self, id: &str) -> Result<DiskSnapshot, RepositoryError>;
}
```

Essas assinaturas são contratos ilustrativos a refinar; o crate do domínio não importa bibliotecas específicas de Windows, Linux ou Tauri.

## 9. Planejador e fila de operações

**Fluxo MVP:** selecionar alvo → adicionar tarefa de simulação → validar → calcular dependências → produzir novo snapshot → exibir diferenças → permitir remover/reordenar/desfazer → exportar plano.

**Validações:** existência/identidade do alvo; capacidade e espaço; alinhamento; limites GPT/MBR; setores; preservação de metadados; proteção de partições; FS identificado; dependências acíclicas; conflitos; tamanhos mínimos; objetivos virtuais; layouts previamente alterados por tarefas anteriores.

**Fluxo de escrita futuro:** confirmar backup e riscos → revisar plano → elevar serviço sob demanda → revalidar disco e capacidades → congelar plano → adquirir exclusividade adequada → persistir journal → executar tarefas em sequência → verificar invariantes pós-condição → registrar sucesso ou falha recuperável. Cancelamento só nos pontos seguros e documentados.

## 10. Segurança, integridade e recuperação futuras

1. Serviço privilegiado de menor privilégio; autenticação IPC, allowlist de ações e não aceitação de comandos shell arbitrários.
2. Verificação de identidade de disco antes e durante fases críticas: serial/WWN quando disponível, GUID/assinatura, tamanho, geometria e fingerprint; não confiar somente em `Disk 1` ou `/dev/sdb`.
3. Bloqueio de partições de boot, EFI, recovery, raiz e volumes montados sem caminho específico offline aprovado.
4. Reconhecimento de BitLocker, LUKS, LVM, RAID, dispositivos removíveis e tecnologias que alteram o modelo tradicional.
5. Pré-verificação de integridade com ferramenta adequada; não presumir que `chkdsk`/`fsck` é sempre seguro em volume montado.
6. Backup externo recomendado e confirmação explícita; VSS/snapshots não substituem backup universal.
7. Journal durável com `operation_id`, estado, versão de esquema, fingerprint, intenção, checkpoints, hashes quando aplicável e pós-condições.
8. Copiador por blocos futuro com operações idempotentes quando possível, sobreposição de origem/destino tratada, buffers alinhados, flush/sync e verificações pós-cópia.
9. Definir política explícita para falta de energia, falha de I/O, desconexão, hibernação, suspensão, remoção USB e interrupção.
10. Separar `failed_safe`, `needs_recovery`, `unsupported_recovery` e `completed_verified`; nunca ocultar incerteza.
11. Não alterar tabela de partições se o plano não puder preservar os dados conforme a estratégia declarada.
12. Escritas devem ser impossíveis de invocar no binário do MVP 0.1, não apenas desabilitadas visualmente.

## 11. Plano de testes e validação

### 11.1 Automação desde o MVP

- Testes unitários: intervalos, overflow, aritmética 512/4096 bytes, alinhamento, capacidade, GPT/MBR, lacunas e dependências.
- Property-based tests: não sobreposição, conservação de capacidade, determinismo, `simulate` sem mutação do snapshot original.
- Golden fixtures: GPT com EFI/MSR/recovery, MBR estendido, vários FS, espaço fragmentado, IDs repetidos, layout inválido, ausência de permissões.
- Contratos dos adaptadores: mesmo schema e semântica; isolamento de erros parciais.
- UI: escala e rotulagem, acessibilidade, navegação, estados vazios, seleção e fila.
- Teste de segurança: pesquisar chamadas de escrita em caminhos de produção do MVP; teste funcional sem elevação.
- CI: format, lint, clippy, testes, build e smoke test em Windows/Linux.

### 11.2 Fases com gravação

- Somente discos virtuais descartáveis inicialmente (VHDX/QCOW2/RAW conforme ambiente), em VMs isoladas; identificação por allowlist explícita.
- Validar GPT/MBR antes e depois, sistema de arquivos, hashes/amostras verificadas, mounting, fsck/chkdsk adequados e boot quando pertinente.
- Injetar falhas entre checkpoints: kill, reinicialização, EIO, disco cheio, cancelamento e mudança de topologia.
- Reproduzir matrix Windows/Linux, tamanho de setor, SSD virtual, USB virtual e volumes criptografados apenas quando suportados.
- Habilitar discos físicos somente após evidência documentada e processo de liberação independente.

## 12. Roadmap e gates de lançamento

| Marco | Escopo | Gate obrigatório |
|---|---|---|
| 0.1 — MVP | Inventário, mapa visual, snapshots, mock, fila e simulação, logs/exportação | Zero escrita; testes do domínio e UI passam em Windows/Linux |
| 0.2 — Escrita controlada | Serviço privilegiado, criar/excluir/formatar em discos virtuais | Laboratório isolado, autorização por alvo, journal mínimo e pós-verificação |
| 0.3 — Resize | Operações suportadas por FS/OS, online/offline conforme matriz | Integridade verificada, testes de limites e falhas |
| 0.4 — Clone/cópia | Copiar partição/disco, validações de origem/destino | Verificação de dados e metadados; recuperação documentada |
| 0.5 — Move | Engine de movimentação e retomada | Fault injection extensa e recuperação de interrupções |
| 0.6 — Merge/Split | Migração de arquivos, metadados, ACLs e atributos | Integridade, permissões e cenários de conflito testados |
| 0.7 — Conversões | Apenas conversões suportadas e elegíveis | Matriz específica; caminho de backup/recuperação |
| 0.8 — Offline | WinPE/Live Linux, tarefas em volumes de SO | Boot, volumes críticos, criptografia e recuperação verificados |
| 1.0 | Endurecimento, instaladores, documentação e compatibilidade publicada | Auditoria de riscos e release checklist aprovado |

Os marcos não implicam prazo nem garantia de que toda operação será viável em todo FS. Adiar recurso inseguro é preferível a simular compatibilidade.

## 13. Critérios não funcionais

- **Confiabilidade:** estado parcial e erros sempre visíveis; logs correlacionáveis; nenhuma alegação de sucesso sem verificação.
- **Desempenho:** descoberta assíncrona e cancelável; UI responsiva com múltiplos discos; sem varredura integral de blocos no inventário.
- **Segurança:** mínima superfície privilegiada, validação em fronteiras, dependências auditáveis e empacotamento assinado quando disponível.
- **Portabilidade:** domínio agnóstico ao SO; adaptadores versionados e features específicas detectadas em runtime.
- **Privacidade:** operação offline por padrão; nenhuma telemetria remota obrigatória; logs sanitizados e exportação explícita.
- **Acessibilidade:** teclado, contraste, descrição textual das cores, leitores de tela e PT-BR; arquitetura de i18n.
- **Manutenibilidade:** documentação de decisões (ADRs), contratos tipados, testes e versionamento do schema.

## 14. Entregáveis da primeira iteração

1. Workspace Cargo com crates separados; app Tauri 2 com React/TypeScript.
2. Definições de entidades, tipos de IDs, setores, intervalos, snapshots e erros estruturados.
3. `StorageProvider`, `OperationPlanner`, `SnapshotRepository` e adaptador Mock.
4. Fixtures sintéticas representativas de GPT, MBR e estados inválidos.
5. UI com lista, mapa de partições e propriedades dos dispositivos.
6. Fila de operações de simulação com preview antes/depois e validação.
7. Testes unitários, propriedades, frontend e pipelines CI básicos.
8. README de instalação, desenvolvimento e limites de segurança.
9. Matriz inicial de capacidades: inventário suportado, escrita real desabilitada.
10. Demonstração verificável de que o binário MVP não possui executor de escrita.

## 15. Instrução operacional para a IA desenvolvedora

Atue como arquiteto e implementador, entregando incrementos pequenos, compiláveis e testados. Antes de editar, apresente breve plano e arquivos envolvidos; implemente um módulo por vez; explique critérios de aceite e evidências de teste. Não crie todo o sistema em uma única resposta. Não habilite gravações reais durante o MVP 0.1. Distinga protótipo, implementação compilada, testes de simulação e validação em hardware. A cada milestone, atualize esta definição e registre decisões arquiteturais.

## 16. Referências técnicas fundamentais

- Microsoft — [Virtual Disk Service e sua substituição pela Storage Management API](https://learn.microsoft.com/en-us/windows/win32/vds/virtual-disk-service-portal).
- Microsoft — [MSFT_Partition](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/msft-partition).
- Microsoft — [IOCTL_DISK_GET_DRIVE_LAYOUT_EX](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_disk_get_drive_layout_ex).
- GNU — [GNU Parted / libparted](https://www.gnu.org/software/parted/manual/parted.html).
- GNU — [resizepart não altera o sistema de arquivos](https://www.gnu.org/software/parted/manual/html_node/resizepart.html).
- Linux — [lsblk(8)](https://man7.org/linux/man-pages/man8/lsblk.8.html), [blkid(8)](https://man7.org/linux/man-pages/man8/blkid.8.html), [findmnt(8)](https://man7.org/linux/man-pages/man8/findmnt.8.html).

---

**Estado do documento:** definição de escopo e arquitetura. As funções avançadas estão especificadas para planejamento, não implementadas nem certificadas para uso em dados reais.

## 17. Estado de implementação (atualizado em 2026-10-28)

- **MVP 0.1 implementado:** inventário (mock; leitura real Windows/Linux por ação do usuário, somente leitura), mapa proporcional, tabela, propriedades com espaço usado/livre, fila simulada (criar, excluir, redimensionar, mover, formatar, clonar disco e partição), desfazer/refazer/reordenar, antes/depois, exportação/importação de snapshot e plano (schema versionado), Aplicar desabilitado.
- **Matriz de capacidades:** `docs/capabilities.md`. **Modelo de ameaças:** `docs/security/threat-model.md`. **Decisões:** `docs/adr/`.
- **Pendente:** CI (GitHub Actions), testes por propriedade, adaptadores validados em hardware, ícone para 16/32 px, auditoria WCAG, e todas as fases com escrita (0.2+), que dependem de autorização do proprietário.

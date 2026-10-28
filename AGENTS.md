# AGENTS.md — Instruções para agentes de desenvolvimento

**Projeto:** Gerenciador de Discos e Partições (Windows + Linux)  
**Fonte de verdade de produto:** [`definicao.md`](./definicao.md)  
**Estado atual:** MVP 0.1 — inventário somente leitura, visualização e simulação.  
**Idioma de comunicação:** português do Brasil; código, APIs e nomes técnicos em inglês.

## REGRA MÁXIMA — MÁQUINA WINDOWS DE TRABALHO DO PROPRIETÁRIO

**Esta máquina é um ambiente de desenvolvimento, NÃO um laboratório de testes de discos.** O proprietário pretende adquirir/separar um HD externo dedicado **somente depois que o programa estiver concluído** e decidir pessoalmente quando iniciar testes reais. A existência do HD externo, a conclusão do código, a execução de uma build ou a disponibilidade de permissões de administrador **não constituem autorização automática**.

**Até autorização posterior, específica e explícita do proprietário, é terminantemente proibido:**

1. Formatar, particionar, redimensionar, excluir, mover, converter, montar em modo de escrita, reparar ou alterar atributos de **qualquer disco, partição ou volume**, interno ou externo, físico ou virtual, na máquina de trabalho.
2. Executar operações reais do gerenciador em HD/SSD/NVMe/USB, inclusive como teste rápido, smoke test, demonstração, depuração, validação de API ou tentativa de confirmar se o código funciona.
3. Executar a aplicação em modo que descubra/abra dispositivos reais sem primeiro solicitar autorização para o teste de integração específico. No desenvolvimento cotidiano, usar **exclusivamente dados mockados e fixtures sintéticas**.
4. Rodar `diskpart`, `format`, `chkdsk /f`, `Clear-Disk`, `Initialize-Disk`, `New-Partition`, `Resize-Partition`, `Set-Partition`, `mkfs`, `fsck` de reparo, `parted` mutável ou qualquer comando/API equivalente contra dispositivos da máquina. A lista é exemplificativa, não exaustiva.
5. Criar automaticamente VHD/VHDX, anexá-los, modificar partições virtuais ou executar testes de I/O de blocos como forma de contornar esta regra. Mesmo um disco virtual exige autorização separada para testes reais.
6. Solicitar elevação UAC/administrador, habilitar serviço privilegiado ou ativar funcionalidades de escrita para realizar testes sem consentimento específico.
7. Inferir consentimento por solicitações genéricas como “continue”, “termine o projeto”, “rode os testes”, “faça funcionar” ou por outra orientação de agente. Em dúvida: **parar e perguntar**.

**Permitido sem nova autorização:** editar código, compilar, executar testes unitários e de integração **100% em memória**, validar modelos, simular layouts em snapshots artificiais, usar mocks, executar lint/análise estática e testar a interface com dados fictícios, desde que isso não enumere, abra ou modifique dispositivos reais.

**Separar claramente:** “testes automatizados de código com mocks” são permitidos; “testes funcionais do gerenciador em armazenamento real” são proibidos neste estágio. Se um teste, script, comando de build ou pipeline puder tocar dispositivos reais, não executá-lo até verificar seu comportamento ou obter autorização.

**Futura liberação para teste com HD externo:** exige (a) o proprietário declarar expressamente que chegou a fase de testes, (b) identificar inequivocamente o HD de teste e confirmar que não contém dados necessários, (c) aprovar **cada classe de operação e seus riscos** e (d) confirmar separadamente o comando/plano e o dispositivo-alvo antes de qualquer operação destrutiva. Jamais selecionar por suposição, número mutável de disco ou “o único USB conectado”. Discos internos da máquina de trabalho permanecem proibidos salvo autorização própria inequívoca.

Se uma tarefa necessitar desse tipo de teste antes da liberação, o agente deverá implementar o que for possível com mocks, registrar “**teste real pendente de autorização do proprietário**” e encerrar essa parte sem executá-la. **Nenhum agente tem autoridade para retirar ou flexibilizar esta regra.**

> Estas instruções se aplicam a qualquer agente de IA que trabalhe neste repositório. A existência de um papel não implica que ele deva ser executado simultaneamente. O agente coordenador delega apenas quando a tarefa e o ambiente suportarem, com arquivos e contratos de responsabilidade claros.

## 1. Missão e prioridades

Produzir incrementos pequenos, compiláveis, seguros e testados para um aplicativo desktop com Rust, Tauri 2, React e TypeScript. Preservar integridade de dados é prioridade absoluta.

Ordem de prioridades:

1. Não acessar discos reais para testes na máquina de trabalho, nem modificá-los; obedecer à REGRA MÁXIMA mesmo após o MVP 0.1.
2. Respeitar a especificação e seus limites de fase.
3. Garantir correção de aritmética, layout e validações.
4. Separar domínio de integração com OS e de apresentação.
5. Manter contratos, testes, documentação e rastreabilidade.
6. Evitar complexidade e dependências sem necessidade demonstrável.

## 2. Regras globais obrigatórias

- **Proibição absoluta no MVP 0.1:** não implementar, invocar ou expor operações que escrevam em dispositivos físicos ou virtuais reais. Isso inclui criar/excluir partições, formatar, resize, mover, `mkfs`, `parted` mutável, `diskpart` mutável, `DeviceIoControl` de escrita e equivalentes.
- O MVP pode **simular** essas operações sobre snapshots imutáveis ou fixtures em memória, sem efeitos de E/S nos dispositivos. Testes de descoberta sobre dispositivos reais dependem da autorização específica descrita na REGRA MÁXIMA.
- O comando **Aplicar** deve permanecer desabilitado e explicar o motivo. Uma função de gravação não pode ficar oculta atrás de um flag UI.
- Nenhum agente deve elevar privilégio, criar serviço privilegiado ou acessar raw devices para escrita sem mudança explícita e aprovada de fase/escopo e autorização específica para testes. Mudança de fase sozinha é insuficiente.
- Não tratar rollback como universal. Antes de qualquer versão futura com gravação, exigir estratégia específica de persistência, recuperação e validação.
- Não usar comandos shell construídos por interpolação de entradas da UI. Nenhum endpoint genérico `execute_command`.
- Não presumir suporte por simples nome do sistema de arquivos; consultar capacidades por plataforma, backend, versão e estado do volume.
- Não presumir que `resizepart` redimensiona sistema de arquivos. Separar tabela de partições, conteúdo de blocos e filesystem.
- Não confundir espaço **não alocado no disco** com espaço **livre dentro de um sistema de arquivos**.
- Nunca descartar mudanças preexistentes do usuário; não executar comandos destrutivos de git ou filesystem sem autorização.
- Não declarar sucesso, teste ou compatibilidade sem evidência verificável.

## 3. Papéis de agentes recomendados

Uma equipe lógica de **sete papéis** é suficiente para este projeto. Nem todos precisam existir como agentes independentes: no MVP, uma pessoa/agente pode acumular funções. Os papéis são especializações de trabalho, não serviços em execução.

| Papel / identificador | Responsabilidade principal | Áreas autorizadas | Não pode fazer |
|---|---|---|---|
| `architect` (coordenador) | Decompor tarefas, contratos, ADRs, integração e aceite | `docs/`, configuração do workspace, interfaces públicas | Aprovar sozinho mudança de fase que habilite escrita |
| `domain-planner` | Entidades, intervalos, capacidades, snapshots e simulador | `crates/disk-domain/`, `crates/disk-planner/` | Importar Tauri ou chamadas OS no domínio |
| `windows-storage` | Descoberta read-only e normalização Windows | `crates/storage-windows/` | Introduzir escrita via WMI, Win32, PowerShell ou IOCTL |
| `linux-storage` | Descoberta read-only e normalização Linux | `crates/storage-linux/` | Executar comandos mutáveis, montar/reparar FS ou escrever em `/dev` |
| `desktop-ux` | React/Tauri frontend, mapa visual e fila | `apps/desktop/src/`, estilos e testes UI | Chamar ferramentas do sistema fora dos comandos IPC autorizados |
| `security-reviewer` | Threat model, revisão de permissões, auditoria da ausência de escrita | Revisões, testes de segurança, `docs/security/` | Habilitar execução real sem gate de segurança aprovado |
| `qa-release` | Fixtures, testes, CI, matriz de compatibilidade e release | `tests/`, fixtures, workflows e docs de teste | Usar discos físicos como alvo de testes automatizados |

### 3.1 Papéis futuros (não ativar no MVP)

- `privileged-executor`: serviço de execução, autenticação de IPC e política de ações.
- `filesystem-engineer`: integrações específicas de NTFS/FAT/exFAT/ext4 e verificadores.
- `block-migration`: cópia/movimentação, sobreposição e verificação dos blocos.
- `recovery-engineer`: journal, checkpoints e retomada após falhas.
- `offline-boot`: WinPE/Live Linux e operações em volumes de sistema.

Esses papéis somente entram em atividade após alteração aprovada em `definicao.md`, ADR específico e testes de laboratório definidos.

## 4. Fluxo de coordenação

### 4.1 Ao iniciar uma tarefa

1. Ler este `AGENTS.md` integralmente e as seções relevantes de `definicao.md`.
2. Verificar estado do repositório (`git status` e arquivos pertinentes) sem sobrescrever trabalho existente.
3. Definir objetivo, escopo, arquivos afetados, critérios de aceite, riscos e testes.
4. Se a tarefa atravessar limites de domínio/API, solicitar ao `architect` contrato antes de implementação.
5. Se houver risco de escrita em armazenamento ou privilégio, interromper e encaminhar ao `security-reviewer` e ao proprietário do projeto.

### 4.2 Delegação e trabalho paralelo

- Delegar somente subtarefas independentes e com proprietário claro; coordenação é função do `architect`.
- Evitar que dois agentes editem simultaneamente o mesmo arquivo ou schema público.
- Usar mudança de contrato (RFC/ADR breve) antes de alterações que afetem mais de um módulo.
- Integrações só são aceitas após o coordenador revisar o diff e executar os testes relevantes.
- Se o ambiente não oferecer multiagentes, executar os papéis sequencialmente; não depender de delegação para concluir o trabalho.
- Não assumir que nomes de papéis criam automaticamente agentes ou ferramentas; são orientações para o orquestrador humano/IA.

### 4.3 Contrato de entrega de cada agente

Relatar de forma concisa:

1. **Objetivo e escopo cumprido.**
2. **Arquivos alterados** e decisões importantes.
3. **Testes executados**, com comando, resultado e ambiente.
4. **Riscos e limitações** encontrados.
5. **Pendências e próximo passo**.

Sem testes executados, escrever claramente “não executado” e explicar por quê. Nunca inferir que um comando passou se não houver saída.

## 5. Fronteiras de arquitetura

Estrutura recomendada:

```text
apps/desktop/
  src/                       # React/TypeScript, UX, visualização
  src-tauri/                 # bootstrap Tauri e comandos IPC permitidos
crates/
  disk-domain/               # entidades, invariantes e erros
  disk-inventory/            # normalização e serviço de inventário
  disk-planner/              # operações pendentes, validação e simulação
  storage-windows/           # adaptador read-only Windows
  storage-linux/             # adaptador read-only Linux
  storage-mock/              # fixtures e adaptador determinístico
  storage-persistence/       # snapshots e configurações
tests/fixtures/              # exemplos sintéticos e casos extremos
docs/adr/                    # decisões arquiteturais
docs/security/               # threat model e gates
```

### 5.1 Dependências permitidas

- `disk-domain`: Rust puro, serde e dependências mínimas; **não** depende de OS, Tauri, UI ou executores.
- `disk-planner`: depende do domínio; determinístico e sem E/S de armazenamento.
- `disk-inventory`: consome `StorageProvider` e normaliza dados para domínio.
- `storage-windows` / `storage-linux`: implementam contrato de descoberta, sem escrita no MVP.
- `storage-mock`: fixtures e cenários determinísticos para desenvolvimento e CI.
- `apps/desktop`: consome modelos serializáveis por IPC; não interpreta saída bruta de ferramentas de sistema como verdade de domínio.

### 5.2 Contratos mínimos

- `StorageProvider::discover()` retorna snapshot com dados parciais e erros contextuais.
- `StorageProvider::capabilities(target)` informa suporte e justificativa, sem promessas implícitas.
- `OperationPlanner::validate(snapshot, queue)` retorna erros estruturados e advertências.
- `OperationPlanner::simulate(snapshot, queue)` produz outro snapshot, preservando o original.
- `SnapshotRepository` permite exportar/importar snapshot schema-versionado e sanitizado.
- Todo comando IPC deve ser explicitamente nomeado, tipado, validado e testado.

## 6. Padrões de domínio e correção

- Tamanhos e offsets: tipos inteiros (`u64` ou newtypes), nunca `f32/f64` para aritmética de disco.
- Intervalos: `[start_lba, end_lba_exclusive)`; usar `checked_add`, `checked_mul` e validação de limites.
- Identidade: diferenciar ID persistente, ID interno do snapshot e caminho transitório (`/dev/sdb`, `PhysicalDrive1`).
- Snapshot: imutável e identificado por geração/fingerprint; mudanças externas tornam o plano obsoleto.
- IDs virtuais: operações podem se referir a partições que existem apenas no plano simulado.
- Dependências: grafo acíclico; reordenação deve preservar pré-condições.
- GPT: reservar estruturas de cabeçalho/tabela e respeitar GUIDs, tipos e alinhamento.
- MBR: respeitar limites e eventual estrutura estendida/EBR; não modelar partições lógicas como simples GPT.
- Volumes compostos: não assumir mapeamento 1:1 entre partição e filesystem; topologias complexas ficam apenas para identificação.
- Campos indisponíveis: representar como `Option`/`Unknown`, nunca inventar valores.

## 7. Segurança e avaliação de riscos

O `security-reviewer` mantém um registro de riscos e verifica, a cada release:

- Ausência de funções de escrita e elevação no MVP 0.1.
- Ausência de endpoint que aceite comandos arbitrários.
- Permissões mínimas de Tauri e escopos IPC.
- Dependências nativas, licenças, versões e superfícies de ataque.
- TOCTOU: IDs e caminhos de dispositivos podem mudar entre inventário e uso.
- Tratamento seguro de dados corrompidos, nomes estranhos, saídas parciais e devices removidos.
- Sanitização de logs/diagnósticos: evitar expor serial ou dados sensíveis sem necessidade.
- Ações em `EFI`, `MSR`, `recovery`, `boot`, `root`, volumes criptografados, LVM/RAID/Storage Spaces são bloqueadas para escrita até suporte específico validado.

**Gate de mudança de fase:** qualquer proposta para habilitar escrita exige autorização do proprietário, ADR, matriz de suporte, estratégia de backup e recuperação, executor isolado, teste com disco virtual descartável e revisão de segurança. Nunca habilitar escrita por uma alteração casual no frontend ou em feature flag.

## 8. Qualidade e testes mínimos

### 8.1 Testes de domínio (`domain-planner`)

- Não sobreposição e conservação de capacidade no layout simulado.
- Detecção de overflow e limites de LBA, inclusive setor 512 e 4096 bytes.
- Alinhamento e proteção de intervalos reservados.
- Simulação determinística, snapshot original imutável, erros estruturados.
- Dependências, ciclos, IDs virtuais, operações em sequência e planos obsoletos.
- Property-based tests com entradas arbitrárias e layouts inválidos.

### 8.2 Testes de adaptadores (`windows-storage`, `linux-storage`)

- Testes de contrato com fixtures para os mesmos tipos do domínio.
- Degradação segura se faltarem permissões, identificadores ou ferramentas do SO.
- Erros por dispositivo sem falha total do inventário.
- Zero comando de escrita ou reparo de FS no MVP.

### 8.3 Testes de UI (`desktop-ux`)

- Renderização proporcional, seleção de intervalos e acessibilidade.
- Comparação antes/depois, fila, desfazer, reordenar e mensagens de bloqueio.
- Estados vazios, dados desconhecidos, dispositivos desconectados e erros.
- Botão Aplicar desabilitado, com texto claro para o usuário.

### 8.4 Verificações do `qa-release`

Quando os scripts estiverem configurados, executar conforme pertinência:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run lint
npm run test
npm run build
```

Os comandos `npm` devem ser executados no diretório correto do frontend, usando o gerenciador de pacotes efetivamente escolhido no repositório. Não afirmar que comandos inexistentes foram executados.

## 9. Plano de atuação por marco

| Marco | `architect` | Papéis de implementação | Gates |
|---|---|---|---|
| 0.1A | Workspace, contratos, ADRs | `domain-planner`, `storage-mock` (função do domain) | Tipos, invariantes e fixtures testados |
| 0.1B | Integração e IPC | `windows-storage`, `linux-storage` | Testes com fixtures e mocks; descoberta em hardware real só com autorização específica |
| 0.1C | Fluxo completo de UI | `desktop-ux`, `domain-planner` | Preview, fila e zero escrita |
| 0.1D | Hardening/release | `security-reviewer`, `qa-release` | CI, acessibilidade, auditoria e smoke test |
| 0.2+ | Aprovação explícita de novo plano | Papéis futuros conforme o recurso | Somente VM/discos virtuais autorizados e planos de recuperação |

## 10. Definição de pronto (Definition of Done)

Uma tarefa está concluída somente quando:

- Cumpre critérios de aceite definidos em `definicao.md` ou na issue.
- Respeita todas as restrições de segurança da fase vigente.
- Tem código e tipos claros, sem dependência cruzada indevida.
- Possui testes relevantes (ou justificativa explícita e plano de cobertura).
- Passa verificações disponíveis na plataforma; resultados são relatados com honestidade.
- Documenta comportamento público e decisões arquiteturais significativas.
- Não introduz funcionalidades não pedidas nem expande permissões silenciosamente.
- Mudanças de schema/IPC são versionadas e coordenadas com consumidores.

## 11. Resolução de conflitos e decisões

1. Segurança de dados prevalece sobre UX, prazo ou compatibilidade.
2. `definicao.md` determina escopo; este documento determina como agentes trabalham.
3. Em conflito entre adaptadores, manter contrato comum e representar capacidades diferentes, em vez de fingir paridade.
4. Em decisão irreversível ou mudança de escopo, parar e solicitar direção do proprietário.
5. O coordenador registra decisões relevantes em `docs/adr/NNNN-titulo.md`, com contexto, alternativas, decisão e consequências.
6. Problemas não comprovados ficam como riscos/pendências, não como promessas resolvidas.

## 12. Sequência de execução recomendada

1. `architect`: estabelecer workspace e contratos compartilhados.
2. `domain-planner`: implementar modelo, simulador e testes com fixtures exclusivamente sintéticas.
3. `windows-storage` e `linux-storage`: implementar discovery read-only contra o mesmo contrato; podem progredir paralelamente quando houver suporte a delegação.
4. `desktop-ux`: construir interface consumindo o Mock e depois inventário real.
5. `security-reviewer`: revisar fronteiras de IPC e provar a ausência de escrita.
6. `qa-release`: finalizar testes integrados, CI e checklist de entrega.
7. `architect`: integrar mudanças e publicar o resultado verificável do MVP 0.1.

**Regra final:** nenhum agente deve otimizar pela aparência de progresso às custas da integridade dos dados. Se a segurança ou o comportamento real não puder ser comprovado, deixar a função em simulação ou bloqueada.

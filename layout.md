# Layout e experiência do usuário — Gerenciador de Partições

**Versão:** 1.0 — proposta inicial de UI/UX  
**Plataformas:** Windows 10/11 e Linux Desktop  
**Implementação:** Tauri 2 + React + TypeScript + Tailwind CSS + shadcn/ui  
**Documentos complementares:** [`definicao.md`](./definicao.md) e [`AGENTS.md`](./AGENTS.md)  
**Fase:** MVP 0.1 — somente leitura e simulação, sem acesso de escrita a discos.

## 1. Conceito visual

Uma ferramenta profissional, moderna e confiável, com personalidade própria. O produto deve transmitir **clareza, controle, precisão e segurança**. A identidade da “batata quente sendo cortada” adiciona originalidade, mas a área de gerenciamento precisa manter aparência séria e técnica.

**Direção:** visual de aplicação desktop de produtividade, com densidade de informação ajustável, superfícies discretas, hierarquia consistente, ícones simples e mapa de partições como protagonista.

**Princípio UX:** o usuário nunca deve confundir o que **existe no disco** com o que **foi apenas planejado**, nem confundir ações simuladas com alterações reais.

## 2. Ícone e identidade

### 2.1 Arquivo oficial atual

- **Nome:** `icone-batata-quente.png`
- **Localização no projeto:** [`images\icon\Batataquenteparticionada.png`](./images\icon\Batataquenteparticionada.png)
- **Caminho absoluto deste workspace de especificação:** `/workspace/scratch/123dd2df7207/images\icon\Batataquenteparticionada.png`
- **Resolução de origem:** 1254 × 1254 px
- **Formato:** PNG RGBA com fundo transparente
- **Localização do arquivo persistente:** [Ícone na Biblioteca](https://chatgpt.com/library?file_id=libfile_04821e7909e0819184b1eb502095bdc0&account_id=personal)

Ao montar o repositório Windows, copiar o arquivo original para `images\icon\Batataquenteparticionada.png` ou `apps/desktop/src/images\icon\Batataquenteparticionada.png`, mantendo referência estável no código. O caminho absoluto acima corresponde apenas ao workspace em que este documento foi preparado; **não** deve ser codificado como caminho de produção na máquina Windows.

### 2.2 Aplicações do ícone

- Ícone do aplicativo, janela, instalador, atalhos e tela Sobre.
- O ícone grande pode conter detalhes e vapor; criar uma versão simplificada para 16/24/32 px, removendo detalhes que causem ruído.
- Gerar posteriormente `icon.ico` (Windows), `32x32.png`, `128x128.png`, `128x128@2x.png` e demais tamanhos exigidos pelo empacotamento Tauri.
- Não utilizar a batata como indicador de operações perigosas: erros e advertências devem empregar símbolos convencionais e texto claro.
- Não usar o nome/logotipo PartitionMagic como identidade do produto.

## 3. Estrutura da janela principal

### 3.1 Regiões

1. **Topo:** título e logo, atualização do inventário, acesso a configurações e indicador visível de modo Simulação.
2. **Navegação esquerda:** discos disponíveis com capacidade, tipo e estado; filtros e busca.
3. **Área central superior:** detalhes resumidos do disco selecionado e mapa de partições proporcional.
4. **Área central inferior:** tabela de partições, volumes e espaço não alocado.
5. **Painel direito contextual:** detalhes do item selecionado e operações simuláveis ou impedimentos.
6. **Rodapé expansível:** fila de operações pendentes, erros, avisos e comparação antes/depois.
7. **Barra de status:** estado do inventário, hora da captura, fonte de dados (Mock/Real autorizado), notificações e ausência de escrita no MVP.

### 3.2 Wireframe conceitual

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ [Ícone] Gerenciador de Partições    [MODO SIMULAÇÃO]   [Atualizar] [⚙]       │
├──────────────────┬─────────────────────────────────────┬─────────────────────┤
│ DISCOS           │ Disco selecionado · resumo          │ DETALHES            │
│                  │ Modelo / tamanho / GPT / setores     │ Item selecionado    │
│ • Disco 0        │                                     │ Tamanho / FS / LBA  │
│   1 TB · NVMe    │ [ EFI ][ C: ........ ][ D: ... ][ ]  │ Status / montagem   │
│ • Disco 1        │  Mapa proporcional das regiões      │                     │
│   500 GB · USB   │                                     │ AÇÕES (simulação)   │
│ • Disco 2        │ ┌─────────────────────────────────┐ │ [Criar] [Resize]    │
│   2 TB · SATA    │ │ Partições / volumes / lacunas   │ │ [Mover] [Excluir]   │
│                  │ │ tabela com detalhes             │ │ Bloqueios/avisos    │
│                  │ └─────────────────────────────────┘ │                     │
├──────────────────┴─────────────────────────────────────┴─────────────────────┤
│ FILA PENDENTE:  2 simulações                  [Desfazer] [Limpar] [Comparar] │
│ 1. Reduzir partição D: (simulado)                                         │
│ 2. Criar partição no espaço previsto (simulado)                             │
│ [Aplicar alterações — DESABILITADO NO MVP 0.1]                               │
├──────────────────────────────────────────────────────────────────────────────┤
│ Inventário: mock | Última captura | Nenhuma operação será gravada em discos  │
└──────────────────────────────────────────────────────────────────────────────┘
```

O wireframe mostra hierarquia e fluxo, não define medidas finais ou exige reproduzir caracteres ASCII na interface.

## 4. Navegação e páginas

| Área | MVP 0.1 | Fases futuras |
|---|---|---|
| Visão geral | Contadores de discos, volumes, espaço, avisos | Diagnósticos e histórico |
| Discos e partições | Inventário, mapa, tabela e propriedades | Operações reais suportadas |
| Planejamento | Fila simulada, conflitos, preview, desfazer | Aplicação controlada, checkpoints e progresso |
| Relatórios | Exportação JSON sanitizada | Exportação estruturada e histórico |
| Configurações | Tema, idioma, unidades e comportamento visual | Preferências de segurança e executor |
| Sobre | Versão, licenças, ícone e limites de uso | Canal de atualização e suporte |

**Navegação recomendada:** sidebar curta com “Visão geral”, “Discos”, “Planejamento”, “Relatórios”, “Configurações”. Não criar muitas páginas para tarefas que devem continuar visíveis enquanto o usuário trabalha com um disco.

## 5. Sistema visual (design tokens)

Paleta proposta (ajustável após prototipação):

| Token | Claro | Escuro | Uso |
|---|---|---|---|
| `surface` | `#F8FAFC` | `#0F172A` | Fundo principal |
| `surface-raised` | `#FFFFFF` | `#1E293B` | Cards e painéis |
| `text-primary` | `#0F172A` | `#F8FAFC` | Texto importante |
| `text-muted` | `#475569` | `#CBD5E1` | Metadados |
| `border` | `#CBD5E1` | `#475569` | Separadores |
| `brand` | `#D97706` | `#F59E0B` | Identidade inspirada na batata |
| `selected` | `#2563EB` | `#60A5FA` | Seleção e foco |
| `success` | `#15803D` | `#4ADE80` | Validado/seguro |
| `warning` | `#B45309` | `#FBBF24` | Atenção |
| `danger` | `#B91C1C` | `#F87171` | Risco/erro |

**Importante:** cores do sistema de arquivos são apenas categorias visuais, não indicadores de segurança. Não usar a mesma codificação cromática para tipo FS e estado da operação sem legenda.

### 5.1 Tipografia e medidas

- Fonte UI: `Inter` ou fonte nativa (`Segoe UI` no Windows; equivalente do sistema no Linux).
- Texto principal 14–15 px; tabelas densas 13–14 px, com opção de densidade confortável.
- Escala de espaçamento de 4 px; áreas clicáveis >= 32 px, preferencialmente 40 px para controles principais.
- Sidebar entre 220 e 260 px; painel de detalhes entre 280 e 340 px; painel de fila redimensionável.
- Layout adaptativo para 1280 × 720 em diante. Em larguras menores, recolher sidebar e mover detalhes para drawer.
- Usuário pode ajustar divisores e persistir preferências de tamanho de painéis.

## 6. Mapa de discos e partições

### 6.1 Regras de representação

- Um disco corresponde a uma barra linear cuja largura total representa a capacidade endereçável, com áreas reservadas modeladas quando relevantes.
- Cada partição ocupa largura proporcional a `length_sectors`; lacunas representam regiões não alocadas.
- Cálculo de layout com números inteiros / razões para não introduzir imprecisão nos offsets; o resultado em pixels pode ser arredondado apenas na renderização.
- Regiões muito pequenas devem continuar selecionáveis: usar largura visual mínima e sinalização de **escala distorcida**, preservando tooltip com tamanho real. Oferecer zoom ou visualização detalhada da região.
- Tooltip mostra nome, tipo, tamanho, offset inicial/final, FS, utilização e estados conhecidos.
- Ao selecionar partição na barra, selecionar a linha correspondente; o inverso também deve funcionar.
- Exibir lacunas explicitamente como “Não alocado”, sem confundir com espaço livre de NTFS/ext4.
- Representar FS desconhecido e criptografia bloqueada com legenda apropriada.
- Evitar representar discos físicos e volumes lógicos no mesmo nível sem indicação de relação.

### 6.2 Categoria visual recomendada

| Categoria | Representação sugerida |
|---|---|
| NTFS | Azul com rótulo NTFS |
| FAT32 | Verde-água com rótulo FAT32 |
| exFAT | Turquesa com rótulo exFAT |
| ext4 | Roxo com rótulo ext4 |
| EFI/MSR/Recovery | Tonalidade neutra + ícone/etiqueta especial |
| Não alocado | Cinza com padrão pontilhado |
| Desconhecido/criptografado | Padrão hachurado com texto descritivo |
| Região simulada | Contorno tracejado e selo “PREVISTO” |

## 7. Fluxo de planejamento de operações

**MVP 0.1:**

1. Selecionar um disco/partição ou espaço não alocado.
2. Abrir operação em modal/painel com campos validados, incluindo unidades e preview.
3. Acionar **“Adicionar à simulação”** — não “Executar”.
4. Recalcular o layout previsto e a lista de dependências.
5. Exibir diferenças entre estado atual e previsto, com advertências.
6. Permitir reordenar/remover/desfazer ações simuladas sem E/S no dispositivo.
7. Exibir “Aplicar indisponível: modo de desenvolvimento seguro” permanentemente.

**Futuro com operações físicas:** confirmação por alvo, backup, pré-condições, journal, revalidação e progresso somente após liberação técnica e autorização do proprietário. Nunca adicionar botão de escrita funcional por simples preferência visual.

### 7.1 Estados visuais de tarefa

`draft` → `pending` → `validating` → `ready` / `blocked` / `cancelled` (todos de planejamento). Não misturar com `running`, `failed`, `recovering` e `completed_verified`, reservados a execução futura.

### 7.2 Rótulos e microcopy obrigatórios

- Banner: **“Modo simulação — nenhuma alteração será gravada em discos.”**
- Botão de simulação: **“Adicionar à fila simulada”**.
- Botão físico indisponível: **“Aplicar alterações (indisponível nesta versão)”**.
- Erro de limite: **“A operação excede o espaço disponível no disco.”**
- Conflito: **“Esta tarefa depende de uma operação anterior.”**
- Alteração externa detectada (em inventário autorizado): **“O layout mudou. Atualize o inventário para recalcular a simulação.”**
- Ação crítica futura: informar disco, identificadores, volumes afetados, consequências e irreversibilidade quando aplicável.

## 8. Formulários e componentes

### 8.1 Componentes React reutilizáveis

- `AppShell`, `SidebarNav`, `DiskList`, `DiskSummaryCard`, `DiskMap`, `PartitionSegment`, `PartitionDetails`, `PartitionTable`.
- `OperationDrawer`, `OperationQueue`, `OperationRow`, `OperationPreview`, `LayoutDiff`, `ValidationBanner`.
- `CapacityDisplay`, `FilesystemBadge`, `CapabilityBadge`, `RiskCallout`, `EmptyState`, `LoadingSkeleton`, `StatusBar`.
- `RefreshInventoryButton` que respeita o bloqueio de acesso real na máquina de trabalho conforme `AGENTS.md`.

### 8.2 Padrões de entrada

- Exibir tamanhos com unidades selecionáveis (KiB/MiB/GiB/TiB e GB/TB) e valor exato em bytes quando útil.
- Slider de resize é auxiliar; deve haver campo numérico preciso, limites mínimos e máximos e avisos de alinhamento.
- Desabilitar operações com motivo contextual visível (tooltip e mensagem), não apenas botão cinza.
- Toda edição na fila deve suportar cancelamento seguro antes de salvar o plano.
- Filtros, busca e ordenação não devem alterar IDs ou estados das partições.

## 9. Estados de tela e feedback

| Estado | Comportamento |
|---|---|
| Sem discos simulados | Exibir botão de carregar exemplos fictícios |
| Inventário carregando | Skeletons e progresso não bloqueante |
| Falha parcial | Mostrar unidades disponíveis e erro por unidade |
| Sem permissão | Explicar limitação; não solicitar admin automaticamente |
| Dispositivo removido | Marcar snapshot obsoleto; bloquear tarefas dependentes |
| Plano válido | Selo “Simulação válida” e resumo de alterações previstas |
| Plano inválido | Bloqueios listados com alvo e solução possível |
| Tarefa pendente | Não sugerir execução física no MVP |
| Falha de IPC | Erro compreensível com opção de tentar novamente, sem comandos externos arbitrários |

## 10. Acessibilidade e internacionalização

- WCAG 2.2 AA como meta: contraste, foco visível, leitura por teclado e sem dependência exclusiva de cor.
- Gráfico de partições deve oferecer representação textual/tabela equivalente para leitores de tela.
- Elementos interativos com rótulos acessíveis, ordem de tabulação coerente e feedback de validação associado ao campo.
- PT-BR como idioma inicial; textos centralizados para futura tradução.
- Números e unidades seguem locale, mas bytes/LBA internos não dependem de locale.
- Modais devem prender foco, permitir Escape quando seguro e devolver foco ao elemento que os abriu.

## 11. Segurança UX na máquina Windows de trabalho

As instruções de `AGENTS.md` têm precedência sobre demonstrações e testes de interface:

- Desenvolver e demonstrar o MVP com **MockStorageAdapter** e fixtures, sem enumerar ou abrir discos reais na máquina de trabalho sem autorização específica.
- Não criar atalhos ocultos, menus debug ou bindings que acionem formatação, comandos de particionamento ou I/O de escrita.
- Não solicitar UAC e não conectar o frontend a serviço privilegiado nesta fase.
- Mesmo quando o programa estiver concluído, aguardar o proprietário autorizar expressamente o teste em HD externo dedicado; sua presença não libera testes automaticamente.
- Separar “executar testes de UI com mocks” de “testar o aplicativo contra armazenamento real”.

## 12. Protótipo e critérios de aceite

### 12.1 Ordem de implementação

1. Design tokens e `AppShell` em dados fictícios.
2. `DiskList`, `DiskMap` e `PartitionTable` sincronizados.
3. `PartitionDetails`, tooltips e legendas.
4. `OperationDrawer`, fila simulada e regras de validação.
5. `LayoutDiff` antes/depois e tratamento de estados de erro.
6. Acessibilidade, responsividade, testes de UI e revisão de segurança.
7. Somente após autorização específica: integração de descoberta com dispositivos reais.

### 12.2 Definition of Done da interface MVP

- A janela funciona em Windows e Linux com dados mockados.
- Mapa, tabela e detalhes compartilham a mesma seleção.
- Escala e layout não confundem espaço não alocado com livre dentro de FS.
- Nenhuma ação simulada afeta dispositivos.
- Banner de simulação visível e botão físico indisponível.
- Teclado, contraste, temas, erro parcial e resoluções-alvo testados.
- Componentes cobertos por testes e sem operações privilegiadas no frontend.
- Ícone aplicado em janela/protótipo quando tamanhos finais estiverem disponíveis.

## 13. Critérios de evolução visual

Antes de declarar o design finalizado, revisar a UI em ao menos três cenários sintéticos: um SSD GPT com EFI/MSR/Windows, um disco Linux ext4/swap e um disco com muitas partições pequenas e lacunas. Validar a leitura de tamanhos, rótulos, espaços e avisos em modo claro/escuro, com janelas estreitas e amplas.

O objetivo é que uma pessoa entenda em poucos segundos **qual disco está selecionado, qual é o estado real ou simulado, e o que acontecerá com cada operação** — sem nenhuma ambiguidade quanto à segurança.

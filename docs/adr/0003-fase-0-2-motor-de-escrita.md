# ADR 0003 — Fase 0.2: motor de escrita (somente em memória por enquanto)

Status: aceito pelo proprietário em 2026-10-29 (início da fase 0.2; teste real só no pendrive D:, sem dados importantes, identificado por **número de série + tamanho**).

## Decisão
- Novo crate `disk-engine`: trait `BlockDevice` + `MemDevice` (único backend). Não é usado por `src-tauri`; o app continua sem nenhum caminho de escrita e o botão Aplicar segue desabilitado.
- Primeira classe de operação: criar/excluir partição primária em **MBR**.
- Salvaguardas obrigatórias em `execute`: alvo idêntico (serial não vazio + tamanho), aprovação ligada ao hash do plano, validação completa antes de escrever, pré-imagem (journal), releitura verificada e rollback verificado.

## Ainda NÃO autorizado / não feito
- Backend de dispositivo real, elevação, serviço privilegiado: exigem, antes de cada operação, confirmação separada do plano e do alvo (AGENTS.md). Nenhuma execução real ocorreu.
- GPT, formatação, resize, mover, clonar.

## Atualização — backend Windows e ferramenta `gdp-mbr`
- Alvo autorizado pelo proprietário em 2026-10-29: serial `2B7C06071070`, 31.042.043.904 bytes, sem dados importantes. O serial/tamanho é passado explicitamente na linha de comando; nada é fixo no código.
- `disk-engine::win` abre `\.\PhysicalDriveN` e recusa disco não-USB, de boot ou de sistema, e identidade que mude entre abertura e leitura.
- `gdp-mbr` (binário separado, fora do app): padrão é simulação; só grava com `--confirm <hash do plano>`. Não solicita elevação: o proprietário o executa num terminal de administrador.
- Nenhuma execução real foi feita pelo agente.

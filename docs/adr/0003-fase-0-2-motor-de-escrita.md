# ADR 0003 — Fase 0.2: motor de escrita (somente em memória por enquanto)

Status: aceito pelo proprietário em 2026-10-29 (início da fase 0.2; teste real só no pendrive D:, sem dados importantes, identificado por **número de série + tamanho**).

## Decisão
- Novo crate `disk-engine`: trait `BlockDevice` + `MemDevice` (único backend). Não é usado por `src-tauri`; o app continua sem nenhum caminho de escrita e o botão Aplicar segue desabilitado.
- Primeira classe de operação: criar/excluir partição primária em **MBR**.
- Salvaguardas obrigatórias em `execute`: alvo idêntico (serial não vazio + tamanho), aprovação ligada ao hash do plano, validação completa antes de escrever, pré-imagem (journal), releitura verificada e rollback verificado.

## Ainda NÃO autorizado / não feito
- Backend de dispositivo real, elevação, serviço privilegiado: exigem, antes de cada operação, confirmação separada do plano e do alvo (AGENTS.md). Nenhuma execução real ocorreu.
- GPT, formatação, resize, mover, clonar.

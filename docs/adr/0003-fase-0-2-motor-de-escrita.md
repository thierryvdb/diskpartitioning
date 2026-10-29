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

## Evidência do primeiro teste real (executado pelo proprietário, 2026-10-29)
Alvo `2B7C06071070`, plano `delete 0` + `create 0 2048 2097152 07` (hash 14521481411146062566). Saída do `gdp-mbr`: "OK: gravado e verificado por releitura"; MBR de `{0x0C, 8064, 60620928}` para `{0x07, 2048, 2097152}`. Cobre: criar/excluir partição primária em MBR. Não cobre: formatação, GPT, resize, mover, clonar; não foi verificada a visão do Windows após a gravação.

## Formatação (`gdp-format`)
Classe de operação aprovada pelo proprietário: formatar FAT32 e NTFS (em sequência) a partição 1 do alvo `2B7C06071070`. Usa `Format-Volume` do Windows (sem formatador próprio), com script fixo; só `u32` e o enum FAT32/NTFS entram no comando. Reconfere a identidade do disco imediatamente antes (USB, não boot/sistema, serial e tamanho), exige aprovação por hash (disco+serial+tamanho+partição+fs) e verifica o sistema de arquivos depois. Simulação por padrão. Limites: não força desmontagem; sem rollback (formatar é destrutivo por natureza).

## Evidência dos testes reais de formatação (executados pelo proprietário, 2026-10-29)
Alvo `2B7C06071070`, partição 1 (início 1048576, 1073741824 bytes). `gdp-format --fs FAT32` (hash 17385003207237959396): "OK … FAT32 e verificada". Em seguida `--fs NTFS` (hash 4237421650828092899), com o sistema de arquivos anterior confirmado como FAT32 na simulação: "OK … NTFS e verificada". A verificação é a releitura do `Get-Volume` feita pela própria ferramenta.

## Aplicar no app (autorizado pelo proprietário em 2026-10-29: "1")
- Comandos IPC `preview_apply` (só leitura) e `apply_plan`. Só valem para o inventário **real**, disco USB não-boot/não-sistema, MBR de 512 B, e as operações criar/excluir/formatar (FAT32/NTFS); o resto é recusado em `disk_engine::apply::translate`.
- O backend recalcula a prévia na hora de aplicar e exige: serial digitado pelo usuário igual ao do disco, e o hash da prévia (disco+serial+tamanho+passos). Não há elevação automática: o app precisa ser iniciado como administrador pelo proprietário; sem isso a prévia falha ao abrir o disco.
- Formatar uma partição recém-criada usa `Update-Disk` e localiza a partição pelo deslocamento; se o Windows ainda não a enxergar, o app orienta reconectar o disco.
- O botão Aplicar fica desabilitado no mock, com fila vazia ou com operações bloqueadas.
- Teste real do fluxo pelo app: pendente de execução pelo proprietário.

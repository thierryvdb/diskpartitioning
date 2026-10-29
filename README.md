# Global Disk Partitioning

Gerenciador de discos e partições para Windows e Linux (Tauri 2 + Rust + React). **MVP 0.1: somente leitura e simulação** — nenhuma escrita em discos; os dados vêm de fixtures sintéticas. Veja `AGENTS.md`, `DEFINICAO.md` e `layout.md`.

Funções simuladas: criar, excluir, redimensionar, mover, formatar, **clonar disco inteiro** e **clonar partição** (nenhuma grava em disco).

## Requisitos
Rust (stable), Node 20+, e no Windows as Build Tools do Visual Studio (C++) + WebView2. No Linux, dependências do Tauri 2 (webkit2gtk).

## Desenvolvimento
```
npm install
npx tauri dev          # app desktop
npx tauri build        # instaladores (MSI/NSIS no Windows)
cargo test --workspace # domínio e planejador (em memória)
npm test               # interface (IPC simulado)
```

## Licença
GNU GPL v3.0 — ver `LICENSE` e `ATTRIBUTION.md`. Autor original: Thierry Vanden Broucke.

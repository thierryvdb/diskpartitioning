# ADR 0001 — Protótipo inicial em TypeScript (SUPERADO)

Substituído em 2026-10-28: sem toolchain Rust o primeiro protótipo foi escrito em TS. Após instalar Rust + Build Tools, o domínio e o planejador foram portados para Rust (`crates/`) e o TS de domínio foi removido.

# ADR 0002 — Arquitetura atual

- `crates/disk-domain`: tipos (u64 em setores, `checked_add`), trait `StorageProvider`.
- `crates/disk-planner`: validação e simulação determinísticas; teste que varre o código de produção atrás de APIs de escrita.
- `crates/storage-mock`: fixtures sintéticas (única fonte de dados no MVP).
- `src-tauri`: apenas `list_disks` e `simulate_plan`; capability `core:default`; sem comando genérico.
- `src/` (React + TS): só apresentação; tipos em `src/domain/types.ts` espelham o Rust. `src/domain/layout.ts` só calcula espaço não alocado para exibição.
- Sem Tailwind/shadcn/Zustand: a UI atual não precisa deles.
- Limite conhecido: u64 chega à UI como number JSON (exato até 2^53 setores).
- Adaptadores Windows/Linux reais não existem e só entram após autorização do proprietário (AGENTS.md).

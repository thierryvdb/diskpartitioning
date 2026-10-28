import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { Disk, Op } from "../domain/types";
import App from "./App";

// A lógica de simulação é testada no Rust (disk-planner); aqui o IPC é falso.
const disk: Disk = {
  id: "d", model: "Disco teste", bus: "SATA", table: "GPT", sectorSize: 512,
  totalSectors: 10000, firstUsableLba: 0, lastUsableExclLba: 10000,
  partitions: [{ id: "p1", name: "EFI", startLba: 0, lengthSectors: 1000, fs: "fat32", kind: "efi", usedSectors: null }],
};
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args?: { ops: Op[] }) =>
    cmd === "list_disks"
      ? [disk]
      : { disks: [disk], issues: [], blockedOps: (args?.ops ?? []).map((o) => o.id) },
}));

afterEach(cleanup);

it("mostra banner de simulação e Aplicar desabilitado", async () => {
  render(<App />);
  expect(await screen.findByText(/nenhuma alteração será gravada em discos\.$/, { selector: ".banner" })).toBeTruthy();
  expect((screen.getByText(/Aplicar alterações/) as HTMLButtonElement).disabled).toBe(true);
});

it("enfileira exclusão, mostra bloqueio vindo do planejador e desfaz", async () => {
  render(<App />);
  fireEvent.click(await screen.findByLabelText(/^EFI · FAT32/));
  fireEvent.click(screen.getByText("Excluir (simular)"));
  expect(await screen.findByText(/BLOQUEADA/)).toBeTruthy();
  fireEvent.click(screen.getByText("Desfazer"));
  await vi.waitFor(() => expect(screen.queryByText(/BLOQUEADA/)).toBeNull());
});

it("Ajuda → Sobre exibe autoria e licença", async () => {
  render(<App />);
  fireEvent.click(await screen.findByText("Ajuda"));
  fireEvent.click(screen.getByText("Sobre"));
  expect(screen.getByText(/Thierry Vanden Broucke/)).toBeTruthy();
  expect(screen.getByText(/GNU GPL v3.0\.$/)).toBeTruthy();
});

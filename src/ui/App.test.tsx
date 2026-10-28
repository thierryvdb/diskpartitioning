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
const calls: string[] = [];
const inventory = { source: "mock", disks: [disk], errors: [], fingerprint: "f1" };
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args?: { ops: Op[]; real?: boolean }) => {
    calls.push(cmd + (cmd === "load_inventory" ? `:${args?.real}` : ""));
    if (cmd === "get_inventory") return inventory;
    if (cmd === "load_inventory") return { ...inventory, source: args?.real ? "real" : "mock", fingerprint: "f2" };
    return { disks: [disk], issues: [], blockedOps: (args?.ops ?? []).map((o) => o.id) };
  },
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

it("Visão geral mostra contadores do inventário", async () => {
  render(<App />);
  fireEvent.click(await screen.findByText("Visão geral", { selector: ".pages button" }));
  expect(screen.getByText("Discos", { selector: "dt" })).toBeTruthy();
  expect(screen.getByText("Partições", { selector: "dt" }).nextSibling?.textContent).toBe("1");
});

it("Configurações aplica e persiste o tema", async () => {
  render(<App />);
  fireEvent.click(await screen.findByText("Configurações", { selector: ".pages button" }));
  fireEvent.change(screen.getByLabelText("Tema"), { target: { value: "dark" } });
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(localStorage.getItem("prefs")).toContain("dark");
});

it("leitura real exige confirmação explícita antes de chamar o backend", async () => {
  calls.length = 0;
  render(<App />);
  fireEvent.click(await screen.findByText("Ler discos reais…"));
  expect(calls.some((c) => c.startsWith("load_inventory"))).toBe(false);
  fireEvent.click(screen.getByText("Cancelar"));
  expect(calls.some((c) => c.startsWith("load_inventory"))).toBe(false);
  fireEvent.click(screen.getByText("Ler discos reais…"));
  fireEvent.click(screen.getByText("Confirmo: ler discos reais"));
  await vi.waitFor(() => expect(calls).toContain("load_inventory:true"));
  expect(await screen.findByText(/REAL \(somente leitura\)/, { selector: ".status" })).toBeTruthy();
});

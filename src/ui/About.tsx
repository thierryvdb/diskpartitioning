import { useEffect, useRef } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { APP_NAME, REPO_URL } from "../domain/format";

export default function About({ onClose }: { onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => ref.current?.showModal?.(), []); // dialog nativo: prende foco e fecha com Esc
  return (
    <dialog ref={ref} onClose={onClose} aria-labelledby="about-title" className="about">
      <header>
        <img src="/icon.png" alt="" width={72} height={72} />
        <div>
          <h2 id="about-title">{APP_NAME}</h2>
          <p className="muted">Software livre · GNU GPL v3.0 · versão 0.1.0</p>
        </div>
      </header>
      <hr />
      <p><strong>Projeto original desenvolvido por Thierry Vanden Broucke.</strong></p>
      <p>Este software é distribuído sob a licença GNU GPL v3.0.</p>
      <p className="muted">O código pode ser utilizado, estudado, modificado e redistribuído conforme os termos da licença.</p>
      <p className="muted">MVP 0.1: somente leitura e simulação. Nenhuma alteração é gravada em discos.</p>
      <a className="btn block" href={REPO_URL} target="_blank" rel="noopener noreferrer"
        onClick={(e) => { e.preventDefault(); openUrl(REPO_URL).catch(() => {}); }}>Visitar repositório original</a>
      <form method="dialog"><button className="btn">Fechar</button></form>
    </dialog>
  );
}

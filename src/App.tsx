import { useCallback, useEffect, useState } from "react";
import { api, type Proposal, type Status, type StatusDto, type Video } from "./api";
import "./App.css";

type Msg = { kind: "info" | "error" | "success"; text: string } | null;

export default function App() {
  const [status, setStatus] = useState<StatusDto | null>(null);
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [videos, setVideos] = useState<Video[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<Msg>(null);
  const [settings, setSettings] = useState(false);
  const [limit, setLimit] = useState<string>("200");
  const [redo, setRedo] = useState(false);
  const [filter, setFilter] = useState<"all" | "upload" | "live" | "other" | Status>("all");

  const refresh = useCallback(async () => {
    try {
      const [s, p, v] = await Promise.all([
        api.getStatus(),
        api.listProposals(),
        api.listVideos(),
      ]);
      setStatus(s);
      setProposals(p);
      setVideos(v);
    } catch (e) {
      setMsg({ kind: "error", text: String(e) });
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    setMsg(null);
    try {
      await fn();
    } catch (e) {
      setMsg({ kind: "error", text: String(e) });
    } finally {
      setBusy(null);
    }
  };

  const handlePull = () =>
    run("Puxando vídeos do YouTube...", async () => {
      const n = parseInt(limit, 10);
      const count = await api.pull(Number.isFinite(n) && n > 0 ? n : null);
      await refresh();
      setMsg({ kind: "success", text: `${count} vídeos carregados.` });
    });

  const handleGenerate = () =>
    run("Gerando sugestões com o Ollama (pode demorar)...", async () => {
      const result = await api.generate(null, redo);
      setProposals(result);
      await refresh();
      setMsg({ kind: "success", text: `${result.length} sugestões disponíveis.` });
    });

  const handleApply = () =>
    run("Aplicando títulos no YouTube...", async () => {
      const r = await api.apply();
      await refresh();
      setMsg({
        kind: r.failed ? "info" : "success",
        text: `Aplicados: ${r.applied}. Falhas: ${r.failed}.`,
      });
    });

  const editTitle = async (videoId: string, title: string) => {
    setProposals((ps) =>
      ps.map((p) => (p.video_id === videoId ? { ...p, proposed_title: title } : p)),
    );
    try {
      await api.editProposal(videoId, title);
    } catch (e) {
      setMsg({ kind: "error", text: String(e) });
    }
  };

  const changeStatus = async (videoId: string, s: Status) => {
    setProposals((ps) =>
      ps.map((p) => (p.video_id === videoId ? { ...p, status: s } : p)),
    );
    try {
      await api.setStatus(videoId, s);
      const st = await api.getStatus();
      setStatus(st);
    } catch (e) {
      setMsg({ kind: "error", text: String(e) });
    }
  };

  const visible = proposals.filter((p) => {
    if (filter === "all" || filter === "pending" || filter === "approved" || filter === "rejected" || filter === "applied") {
      return filter === "all" || p.status === filter;
    }
    const v = videos.find((x) => x.id === p.video_id);
    if (filter === "upload") return v?.kind === "upload" || v?.kind === "";
    if (filter === "live") return v?.kind === "live";
    return v?.kind !== "upload" && v?.kind !== "live" && v?.kind !== "";
  });
  const approvedCount = proposals.filter((p) => p.status === "approved").length;

  return (
    <div className="app">
      <header>
        <div className="brand">
          Max<span>Tube</span>
        </div>
        <div className="pills">
          <Pill ok={status?.logged_in} label={status?.logged_in ? "Conectado" : "Desconectado"} />
          <Pill ok={!!status?.has_credentials} label="Credenciais" />
          <span className="pill neutral">{status?.model ?? "..."}</span>
          <span className="pill neutral">{status?.videos ?? 0} vídeos</span>
        </div>
        <button className="ghost" onClick={() => setSettings(true)}>
          Configurações
        </button>
      </header>

      <section className="toolbar">
        <label className="inline">
          Limite
          <input
            className="num"
          value={limit}
          onChange={(e) => setLimit(e.target.value)}
          placeholder="ex.: 500 ou vazio (todos)"
        />
        </label>
        <button onClick={handlePull} disabled={!!busy || !status?.logged_in}>
          Puxar vídeos
        </button>
        <button onClick={handleGenerate} disabled={!!busy || status?.videos === 0}>
          Gerar sugestões
        </button>
        <label className="inline checkbox">
          <input type="checkbox" checked={redo} onChange={(e) => setRedo(e.target.checked)} />
          refazer
        </label>
        <div className="spacer" />
        <button className="primary" onClick={handleApply} disabled={!!busy || approvedCount === 0}>
          Aplicar aprovados ({approvedCount})
        </button>
      </section>

      {busy && (
        <div className="banner info">
          <span className="spin" /> {busy}
        </div>
      )}
      {msg && <div className={`banner ${msg.kind}`}>{msg.text}</div>}

      <nav className="filters">
        {([
          "all",
          "pending",
          "approved",
          "rejected",
          "applied",
          "upload",
          "live",
          "other",
        ] as const).map((f) => (
          <button
            key={f}
            className={filter === f ? "tab active" : "tab"}
            onClick={() => setFilter(f)}
          >
            {labelForFilter(f)}
            {countFor(f, proposals, videos) > 0 ? ` (${countFor(f, proposals, videos)})` : ""}
          </button>
        ))}
      </nav>

      <main>
        {visible.length === 0 ? (
          <Empty hasVideos={videos.length > 0} onPull={handlePull} disabled={!!busy} />
        ) : (
          <ul className="list">
            {visible.map((p) => (
              <li key={p.video_id} className={`row ${p.status}`}>
                <div className="original">
                  <span className="tag">Atual</span>
                  <span>{p.original_title}</span>
                  {videos.find((x) => x.id === p.video_id)?.kind && (
                    <span className="meta">
                      tipo: {videos.find((x) => x.id === p.video_id)!.kind}
                    </span>
                  )}
                </div>
                <div className="proposed">
                  <span className="tag">Sugerido</span>
                  <input
                    value={p.proposed_title}
                    maxLength={100}
                    onChange={(e) => editTitle(p.video_id, e.target.value)}
                  />
                  <span className="count">{p.proposed_title.length}/100</span>
                </div>
                <div className="actions">
                  <span className={`badge ${p.status}`}>{labelFor(p.status)}</span>
                  <button
                    className="ok"
                    disabled={p.status === "approved" || p.status === "applied"}
                    onClick={() => changeStatus(p.video_id, "approved")}
                  >
                    Aprovar
                  </button>
                  <button
                    className="no"
                    disabled={p.status === "rejected"}
                    onClick={() => changeStatus(p.video_id, "rejected")}
                  >
                    Rejeitar
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </main>

      {settings && (
        <Settings
          status={status}
          onClose={() => setSettings(false)}
          onSaved={refresh}
          setMsg={setMsg}
        />
      )}
    </div>
  );
}

function labelForFilter(f: any): string {
  return {
    all: "Todas",
    pending: "Pendentes",
    approved: "Aprovadas",
    rejected: "Rejeitadas",
    applied: "Aplicadas",
    upload: "Vídeos",
    live: "Ao Vivo",
    other: "Outros",
  }[f] as string;
}

function countFor(
  f: any,
  proposals: Proposal[],
  videos: Video[],
): number {
  if (f === "all") return proposals.length;
  if (f === "pending" || f === "approved" || f === "rejected" || f === "applied") {
    return proposals.filter((p) => p.status === f).length;
  }
  return proposals.filter((p) => {
    const v = videos.find((x) => x.id === p.video_id);
    if (f === "upload") return v?.kind === "upload" || v?.kind === "";
    if (f === "live") return v?.kind === "live";
    return v?.kind !== "upload" && v?.kind !== "live" && v?.kind !== "";
  }).length;
}

function Pill({ ok, label }: { ok?: boolean; label: string }) {
  return <span className={`pill ${ok ? "ok" : "no"}`}>{label}</span>;
}

function Empty({
  hasVideos,
  onPull,
  disabled,
}: {
  hasVideos: boolean;
  onPull: () => void;
  disabled: boolean;
}) {
  return (
    <div className="empty">
      <h2>Nada por aqui ainda</h2>
      <p>
        {hasVideos
          ? 'Você já tem vídeos carregados. Clique em "Gerar sugestões".'
          : 'Clique em "Puxar vídeos" para baixar os títulos do seu canal.'}
      </p>
      {!hasVideos && (
        <button onClick={onPull} disabled={disabled}>
          Puxar vídeos
        </button>
      )}
    </div>
  );
}

function Settings({
  status,
  onClose,
  onSaved,
  setMsg,
}: {
  status: StatusDto | null;
  onClose: () => void;
  onSaved: () => void;
  setMsg: (m: Msg) => void;
}) {
  const [ollamaUrl, setOllamaUrl] = useState(status?.ollama_url ?? "");
  const [model, setModel] = useState(status?.model ?? "");
  const [prompt, setPrompt] = useState(status?.prompt ?? "");
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [json, setJson] = useState("");
  const [busy, setBusy] = useState(false);

  const wrap = async (fn: () => Promise<void>, okText: string) => {
    setBusy(true);
    try {
      await fn();
      setMsg({ kind: "success", text: okText });
    } catch (e) {
      setMsg({ kind: "error", text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <h2>Configurações</h2>
          <button className="ghost" onClick={onClose}>
            Fechar
          </button>
        </div>

        <section>
          <h3>Ollama (IA local)</h3>
          <label>URL do servidor</label>
          <input value={ollamaUrl} onChange={(e) => setOllamaUrl(e.target.value)} />
          <label>Modelo</label>
          <input value={model} onChange={(e) => setModel(e.target.value)} />
          <button
            disabled={busy}
            onClick={() =>
              wrap(async () => {
                await api.setOllamaUrl(ollamaUrl);
                await api.setModel(model);
                await onSaved();
              }, "Configuração do Ollama salva.")
            }
          >
            Salvar
          </button>
        </section>

        <section>
          <h3>Credenciais do Google</h3>
          <p className="hint">
            Cole o JSON completo do client_secret ou preencha os campos. As credenciais
            ficam em ~/.maxtube.
          </p>
          <textarea
            placeholder="Cole aqui o JSON do client_secret.json"
            value={json}
            onChange={(e) => setJson(e.target.value)}
          />
          <input placeholder="Client ID" value={clientId} onChange={(e) => setClientId(e.target.value)} />
          <input
            placeholder="Client Secret"
            value={clientSecret}
            onChange={(e) => setClientSecret(e.target.value)}
          />
          <div className="row-btns">
            <button
              disabled={busy}
              onClick={() =>
                wrap(async () => {
                  await api.saveCredentials(clientId, clientSecret, json);
                  await onSaved();
                }, "Credenciais salvas.")
              }
            >
              Salvar credenciais
            </button>
            <button
              className="ghost"
              disabled={busy}
              onClick={() =>
                wrap(async () => {
                  await api.login();
                  await onSaved();
                }, "Login concluído.")
              }
            >
              Fazer login com Google
            </button>
          </div>
        </section>

        <section>
          <h3>Prompt principal</h3>
          <p className="hint">
            Placeholders: <code>{"{{title}}"}</code> e <code>{"{{description}}"}</code>
          </p>
          <textarea
            className="prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
          />
          <button
            disabled={busy}
            onClick={() => wrap(() => api.savePrompt(prompt), "Prompt salvo.")}
          >
            Salvar prompt
          </button>
        </section>
      </div>
    </div>
  );
}

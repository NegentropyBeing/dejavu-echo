import { useEffect, useRef, useState } from "react"
import type { Counters, LogEntry, LogLevel } from "../types"

type Props = {
  logs: LogEntry[]
  counters: Counters
}

const LEVEL_LABEL: Record<LogLevel, string> = {
  info: "info",
  ok: "ok",
  warn: "aviso",
  error: "erro",
}

function formatTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleTimeString("pt-BR", { hour12: false })
}

export function LogTab({ logs, counters }: Props) {
  const [filter, setFilter] = useState<LogLevel | "all">("all")
  const [follow, setFollow] = useState(true)
  const endRef = useRef<HTMLDivElement>(null)

  const visible = filter === "all" ? logs : logs.filter((l) => l.level === filter)

  useEffect(() => {
    if (follow) endRef.current?.scrollIntoView({ behavior: "smooth" })
  }, [visible.length, follow])

  const download = () => {
    const text = logs
      .map((l) => `[${formatTime(l.at)}] ${l.level.toUpperCase()} ${l.text}`)
      .join("\r\n")
    const blob = new Blob([text], { type: "text/plain;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = `dejavu-echo-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.log`
    a.click()
    URL.revokeObjectURL(url)
  }

  const stats: Array<[string, number]> = [
    ["Enviadas", counters.sent],
    ["Ignoradas", counters.skipped],
    ["Apagadas", counters.deleted],
    ["Banidos", counters.banned],
  ]

  return (
    <div className="tab-body">
      <section className="stats">
        {stats.map(([label, value]) => (
          <div key={label} className="stat">
            <span className="stat-value">{value}</span>
            <span className="stat-label">{label}</span>
          </div>
        ))}
      </section>

      <section className="card grow">
        <div className="row between">
          <h2>Registro</h2>
          <div className="row gap">
            <select
              value={filter}
              onChange={(e) => setFilter(e.target.value as LogLevel | "all")}
              aria-label="filtrar nível"
            >
              <option value="all">Tudo</option>
              <option value="ok">Sucesso</option>
              <option value="info">Info</option>
              <option value="warn">Avisos</option>
              <option value="error">Erros</option>
            </select>
            <label className="checkbox small">
              <input
                type="checkbox"
                checked={follow}
                onChange={(e) => setFollow(e.target.checked)}
              />
              <span>Acompanhar</span>
            </label>
            <button type="button" onClick={download} disabled={logs.length === 0}>
              Exportar
            </button>
          </div>
        </div>
        <div className="log-view">
          {visible.length === 0 ? (
            <p className="hint">
              Nada por aqui ainda. Inicie a leitura na aba Conexão.
            </p>
          ) : (
            visible.map((l) => (
              <div key={l.id} className={`log-line ${l.level}`}>
                <span className="log-time">{formatTime(l.at)}</span>
                <span className={`log-level ${l.level}`}>{LEVEL_LABEL[l.level]}</span>
                <span className="log-text">{l.text}</span>
              </div>
            ))
          )}
          <div ref={endRef} />
        </div>
      </section>
    </div>
  )
}

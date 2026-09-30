import { useEffect, useState } from "react"
import type { EngineStatus, WebhookState } from "../types"

type Props = {
  status: EngineStatus
  webhook: WebhookState
  channel: string | null
  busy: boolean
  onStart: (channel: string) => void
  onStop: () => void
  onSetWebhook: (url: string) => void
  onClearWebhook: () => void
  onCheckWebhook: () => void
  onOpenDiscord: () => void
  onRevealConfig: () => void
}

const STATUS_TEXT: Record<string, string> = {
  idle: "Parado",
  connecting: "Conectando...",
  live: "Lendo o chat",
  stopped: "Parado",
}

const WEBHOOK_TEXT: Record<WebhookState, string> = {
  unknown: "Não configurado",
  valid: "Configurado e válido",
  invalid: "Excluído ou incorreto",
  unreachable: "Não foi possível verificar",
}

function isRunning(status: EngineStatus): boolean {
  return status.state === "connecting" || status.state === "live"
}

export function ConnectionTab({
  status,
  webhook,
  channel,
  busy,
  onStart,
  onStop,
  onSetWebhook,
  onClearWebhook,
  onCheckWebhook,
  onOpenDiscord,
  onRevealConfig,
}: Props) {
  const [channelInput, setChannelInput] = useState(channel ?? "")
  const [url, setUrl] = useState("")
  const running = isRunning(status)
  const statusClass = status.state === "error" ? "error" : status.state

  useEffect(() => {
    if (channel && !running) setChannelInput(channel)
  }, [channel, running])

  return (
    <div className="tab-body">
      <section className="card">
        <h2>Webhook do Discord</h2>
        <div className={`pill ${webhook === "valid" ? "ok" : webhook === "unknown" ? "" : "warn"}`}>
          {WEBHOOK_TEXT[webhook]}
        </div>
        <p className="hint">
          No Discord: canal → Editar canal → Integrações → Webhooks → Novo
          Webhook → Copiar URL do Webhook. É preciso ter a permissão
          “Gerenciar Webhooks”.
        </p>
        <div className="row">
          <input
            type="password"
            value={url}
            placeholder="https://discord.com/api/webhooks/..."
            onChange={(e) => setUrl(e.target.value)}
            autoComplete="off"
            spellCheck={false}
          />
          <button
            type="button"
            className="primary"
            disabled={busy || !url.trim()}
            onClick={() => onSetWebhook(url.trim())}
          >
            Salvar
          </button>
        </div>
        <div className="row gap">
          <button type="button" onClick={onCheckWebhook} disabled={busy}>
            Verificar salvo
          </button>
          <button type="button" onClick={onClearWebhook} disabled={busy || webhook === "unknown"}>
            Apagar
          </button>
          <button type="button" onClick={onOpenDiscord} disabled={busy}>
            Abrir Discord
          </button>
          <button type="button" onClick={onRevealConfig} disabled={busy}>
            Mostrar pasta do segredo
          </button>
        </div>
        <p className="hint">
          A URL fica num arquivo oculto <code>.secret_env</code> na pasta de
          configuração do aplicativo, nunca na pasta do projeto.
        </p>
      </section>

      <section className="card">
        <h2>Canal da Twitch</h2>
        <div className={`pill ${statusClass}`}>
          {status.state === "error" ? `Erro: ${status.message}` : STATUS_TEXT[status.state]}
          {running && channel ? ` — #${channel}` : ""}
        </div>
        <div className="row">
          <input
            type="text"
            value={channelInput}
            placeholder="nomedocanal ou https://www.twitch.tv/nomedocanal"
            onChange={(e) => setChannelInput(e.target.value)}
            disabled={running}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !running && channelInput.trim()) {
                onStart(channelInput.trim())
              }
            }}
          />
          {running ? (
            <button type="button" className="danger" disabled={busy} onClick={onStop}>
              Encerrar
            </button>
          ) : (
            <button
              type="button"
              className="primary"
              disabled={busy || !channelInput.trim() || webhook === "unknown"}
              onClick={() => onStart(channelInput.trim())}
            >
              Iniciar
            </button>
          )}
        </div>
        {webhook === "unknown" && (
          <p className="hint warn-text">Configure o webhook antes de iniciar.</p>
        )}
        <p className="hint">
          A leitura do chat é anônima: não precisa de conta nem token da Twitch.
        </p>
      </section>
    </div>
  )
}

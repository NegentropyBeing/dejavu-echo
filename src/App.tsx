import { useState } from "react"
import { ConnectionTab } from "./components/ConnectionTab"
import { FilterTab } from "./components/FilterTab"
import { LogTab } from "./components/LogTab"
import { useEngine } from "./useEngine"

type Tab = "conexao" | "filtro" | "registro"

const TABS: Array<[Tab, string]> = [
  ["conexao", "Conexão"],
  ["filtro", "Filtro"],
  ["registro", "Registro"],
]

export default function App() {
  const [tab, setTab] = useState<Tab>("conexao")
  const engine = useEngine()

  return (
    <main className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">déjà vu</span>
          <span className="brand-sub">echo</span>
        </div>
        <nav className="tabs">
          {TABS.map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={tab === id ? "tab active" : "tab"}
              onClick={() => setTab(id)}
            >
              {label}
              {id === "registro" && engine.logs.length > 0 && (
                <span className="badge-count">{engine.logs.length}</span>
              )}
            </button>
          ))}
        </nav>
      </header>

      {engine.error && (
        <div className="banner error">
          <span>{engine.error}</span>
          <button type="button" onClick={engine.dismissError} aria-label="fechar">
            ×
          </button>
        </div>
      )}

      {tab === "conexao" && (
        <ConnectionTab
          status={engine.status}
          webhook={engine.webhook}
          channel={engine.channel}
          busy={engine.busy}
          onStart={engine.start}
          onStop={engine.stop}
          onSetWebhook={engine.setWebhook}
          onClearWebhook={engine.clearWebhook}
          onCheckWebhook={engine.checkWebhook}
          onOpenDiscord={engine.openDiscordSettings}
          onRevealConfig={engine.revealConfigDir}
        />
      )}

      {tab === "filtro" && (
        <FilterTab
          settings={engine.settings}
          busy={engine.busy}
          onSave={engine.saveSettings}
        />
      )}

      {tab === "registro" && (
        <LogTab logs={engine.logs} counters={engine.counters} />
      )}
    </main>
  )
}

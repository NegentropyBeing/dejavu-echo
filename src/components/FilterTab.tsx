import { useEffect, useState } from "react"
import { TagInput } from "./TagInput"
import { ALL_ROLES, ROLE_LABELS, type Role, type Settings } from "../types"

type Props = {
  settings: Settings
  busy: boolean
  onSave: (next: Settings) => void
}

export function FilterTab({ settings, busy, onSave }: Props) {
  const [draft, setDraft] = useState<Settings>(settings)
  const [dirty, setDirty] = useState(false)

  // recarrega o rascunho quando a configuração persistida muda (ex.: após salvar)
  useEffect(() => {
    setDraft(settings)
    setDirty(false)
  }, [settings])

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setDirty(true)
    setDraft((prev) => ({ ...prev, [key]: value }))
  }

  const toggleRole = (role: Role) => {
    const has = draft.allowedRoles.includes(role)
    update(
      "allowedRoles",
      has ? draft.allowedRoles.filter((r) => r !== role) : [...draft.allowedRoles, role],
    )
  }

  const numberHandler =
    (key: "dedupeMinutes" | "userCooldownSeconds" | "delaySeconds", min: number, max: number) =>
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const raw = Number(e.target.value)
      const value = Number.isFinite(raw) ? Math.min(max, Math.max(min, raw)) : min
      update(key, value)
    }

  return (
    <div className="tab-body">
      <section className="card">
        <h2>Quem pode aparecer no Discord</h2>
        <p className="hint">
          Sem ninguém selecionado, apenas o dono do canal é aceito. Ao marcar uma
          função, as mensagens dela passam a ser enviadas.
        </p>
        <div className="roles">
          {ALL_ROLES.map((role) => (
            <label key={role} className="checkbox">
              <input
                type="checkbox"
                checked={draft.allowedRoles.includes(role)}
                onChange={() => toggleRole(role)}
              />
              <span>{ROLE_LABELS[role]}</span>
            </label>
          ))}
        </div>
      </section>

      <section className="card">
        <h2>Links</h2>
        <TagInput
          label="Domínios permitidos (pulam o filtro)"
          hint="Um domínio por vez. Subdomínios são aceitos automaticamente."
          values={draft.allowedDomains}
          placeholder="exemplo.com"
          onChange={(v) => update("allowedDomains", v)}
        />
        <TagInput
          label="Domínios bloqueados"
          hint="Links desses domínios são removidos da mensagem."
          values={draft.blockedDomains}
          placeholder="bit.ly"
          onChange={(v) => update("blockedDomains", v)}
        />
      </section>

      <section className="card">
        <h2>Usuários silenciados</h2>
        <TagInput
          label="Logins ignorados"
          hint="Bots e pessoas cujas mensagens nunca são enviadas."
          values={draft.blockedUsers}
          placeholder="nightbot"
          onChange={(v) => update("blockedUsers", v)}
        />
      </section>

      <section className="card">
        <h2>Tempos</h2>
        <div className="grid-3">
          <div className="field">
            <label className="field-label" htmlFor="dedupe">
              Antiduplicidade (min)
            </label>
            <input
              id="dedupe"
              type="number"
              min={0}
              max={1440}
              value={draft.dedupeMinutes}
              onChange={numberHandler("dedupeMinutes", 0, 1440)}
            />
            <p className="hint">Mesmo link não repete nesse intervalo.</p>
          </div>
          <div className="field">
            <label className="field-label" htmlFor="cooldown">
              Espera por usuário (s)
            </label>
            <input
              id="cooldown"
              type="number"
              min={0}
              max={3600}
              value={draft.userCooldownSeconds}
              onChange={numberHandler("userCooldownSeconds", 0, 3600)}
            />
            <p className="hint">0 desliga a espera entre mensagens.</p>
          </div>
          <div className="field">
            <label className="field-label" htmlFor="delay">
              Atraso antes de enviar (s)
            </label>
            <input
              id="delay"
              type="number"
              min={0}
              max={120}
              value={draft.delaySeconds}
              onChange={numberHandler("delaySeconds", 0, 120)}
            />
            <p className="hint">
              Se a mensagem for apagada nesse tempo, nada é enviado.
            </p>
          </div>
        </div>
      </section>

      <div className="row end">
        {dirty && <span className="hint">Alterações não salvas</span>}
        <button
          type="button"
          className="primary"
          disabled={busy}
          onClick={() => onSave(draft)}
        >
          Salvar configurações
        </button>
      </div>
    </div>
  )
}

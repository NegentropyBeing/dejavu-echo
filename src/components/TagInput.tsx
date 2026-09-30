import { useState } from "react"

type Props = {
  label: string
  hint?: string
  values: string[]
  placeholder?: string
  onChange: (next: string[]) => void
}

export function TagInput({ label, hint, values, placeholder, onChange }: Props) {
  const [draft, setDraft] = useState("")

  const add = () => {
    const next = draft
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean)
    if (next.length === 0) return
    const merged = [...values]
    for (const v of next) {
      if (!merged.includes(v)) merged.push(v)
    }
    onChange(merged)
    setDraft("")
  }

  return (
    <div className="field">
      <label className="field-label" htmlFor={`tag-${label}`}>
        {label}
      </label>
      {values.length > 0 && (
        <ul className="chips">
          {values.map((v) => (
            <li key={v} className="chip">
              <span>{v}</span>
              <button
                type="button"
                aria-label={`remover ${v}`}
                onClick={() => onChange(values.filter((x) => x !== v))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="row">
        <input
          id={`tag-${label}`}
          type="text"
          value={draft}
          placeholder={placeholder}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault()
              add()
            }
          }}
        />
        <button type="button" onClick={add} disabled={!draft.trim()}>
          Adicionar
        </button>
      </div>
      {hint && <p className="hint">{hint}</p>}
    </div>
  )
}

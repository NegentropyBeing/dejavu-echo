import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { useCallback, useEffect, useRef, useState } from "react"
import {
  DEFAULT_SETTINGS,
  type Counters,
  type EngineStatus,
  type LogEntry,
  type Settings,
  type StatusPayload,
  type WebhookState,
} from "./types"

const EMPTY_COUNTERS: Counters = { sent: 0, skipped: 0, deleted: 0, banned: 0 }
const MAX_LOGS = 500

export function useEngine() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS)
  const [status, setStatus] = useState<EngineStatus>({ state: "idle" })
  const [channel, setChannel] = useState<string | null>(null)
  const [webhook, setWebhookState] = useState<WebhookState>("unknown")
  const [counters, setCounters] = useState<Counters>(EMPTY_COUNTERS)
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // mantém o estado do motor sem exigir que o App remonte a cada evento
  const unlisteners = useRef<UnlistenFn[]>([])

  useEffect(() => {
    let cancelled = false

    const bootstrap = async () => {
      try {
        const [loaded, snapshot] = await Promise.all([
          invoke<Settings>("load_settings"),
          invoke<StatusPayload>("engine_status"),
        ])
        if (cancelled) return
        setSettings(loaded)
        setStatus(snapshot.status)
        setChannel(snapshot.channel)
        setWebhookState(snapshot.webhook)
        setCounters(snapshot.counters)
        setLogs(snapshot.logs)
      } catch (e) {
        if (!cancelled) setError(String(e))
      }
    }

    const subs: Promise<UnlistenFn>[] = [
      listen<EngineStatus>("status", (e) => setStatus(e.payload)),
      listen<Counters>("counters", (e) => setCounters(e.payload)),
      listen<LogEntry>("log", (e) =>
        setLogs((prev) => {
          const next = [...prev, e.payload]
          return next.length > MAX_LOGS ? next.slice(next.length - MAX_LOGS) : next
        }),
      ),
    ]

    void bootstrap()
    void Promise.all(subs).then((fns) => {
      if (cancelled) fns.forEach((f) => f())
      else unlisteners.current = fns
    })

    return () => {
      cancelled = true
      unlisteners.current.forEach((f) => f())
      unlisteners.current = []
    }
  }, [])

  const guard = useCallback(async (fn: () => Promise<void>) => {
    setBusy(true)
    setError(null)
    try {
      await fn()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }, [])

  const start = useCallback(
    (channelInput: string) =>
      guard(async () => {
        await invoke("start_engine", { channelInput })
        setChannel(channelInput.trim().replace(/^#/, "").toLowerCase())
        setCounters(EMPTY_COUNTERS)
        setLogs([])
      }),
    [guard],
  )

  const stop = useCallback(
    () =>
      guard(async () => {
        await invoke("stop_engine")
      }),
    [guard],
  )

  const saveSettings = useCallback(
    (next: Settings) =>
      guard(async () => {
        const saved = await invoke<Settings>("save_settings", { settings: next })
        setSettings(saved)
      }),
    [guard],
  )

  const setWebhook = useCallback(
    (url: string) =>
      guard(async () => {
        const state = await invoke<WebhookState>("set_webhook", { url })
        setWebhookState(state)
      }),
    [guard],
  )

  const clearWebhook = useCallback(
    () =>
      guard(async () => {
        await invoke("clear_webhook")
        setWebhookState("unknown")
      }),
    [guard],
  )

  const checkWebhook = useCallback(
    () =>
      guard(async () => {
        const state = await invoke<WebhookState>("webhook_status", { reset: true })
        setWebhookState(state)
      }),
    [guard],
  )

  const openDiscordSettings = useCallback(
    () =>
      guard(async () => {
        await invoke("open_discord_webhook_settings")
      }),
    [guard],
  )

  const revealConfigDir = useCallback(
    () =>
      guard(async () => {
        await invoke("reveal_settings_file")
      }),
    [guard],
  )

  return {
    settings,
    status,
    channel,
    webhook,
    counters,
    logs,
    busy,
    error,
    start,
    stop,
    saveSettings,
    setWebhook,
    clearWebhook,
    checkWebhook,
    openDiscordSettings,
    revealConfigDir,
    dismissError: useCallback(() => setError(null), []),
  }
}

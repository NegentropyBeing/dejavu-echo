export type Role = "broadcaster" | "moderator" | "vip" | "subscriber"

export type Settings = {
  allowedRoles: Role[]
  blockedUsers: string[]
  blockedDomains: string[]
  allowedDomains: string[]
  dedupeMinutes: number
  userCooldownSeconds: number
  delaySeconds: number
}

export type EngineState = "idle" | "connecting" | "live" | "stopped"

export type EngineStatus =
  | { state: EngineState }
  | { state: "error"; message: string }

export type WebhookState = "unknown" | "valid" | "invalid" | "unreachable"

export type LogLevel = "info" | "ok" | "warn" | "error"

export type LogEntry = {
  id: number
  at: string
  level: LogLevel
  text: string
}

export type Counters = {
  sent: number
  skipped: number
  deleted: number
  banned: number
}

export type StatusPayload = {
  status: EngineStatus
  channel: string | null
  counters: Counters
  webhook: WebhookState
  logs: LogEntry[]
}

export const DEFAULT_SETTINGS: Settings = {
  allowedRoles: [],
  blockedUsers: [
    "nightbot",
    "streamelements",
    "streamlabs",
    "moobot",
    "fossabot",
    "wizebot",
  ],
  blockedDomains: [
    "bit.ly",
    "tinyurl.com",
    "t.co",
    "grabify.link",
    "iplogger.org",
    "discord.gg",
  ],
  allowedDomains: [],
  dedupeMinutes: 30,
  userCooldownSeconds: 20,
  delaySeconds: 8,
}

export const ROLE_LABELS: Record<Role, string> = {
  broadcaster: "Dono do canal",
  moderator: "Moderador",
  vip: "VIP",
  subscriber: "Assinante",
}

export const ALL_ROLES: Role[] = [
  "broadcaster",
  "moderator",
  "vip",
  "subscriber",
]

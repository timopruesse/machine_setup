# Secrets substrate; 1Password-first; SSH agent wedge before generic refs

## Status (2026-09-11 — design accepted; Phase 0 implemented)

Wipe/restore often blocks on **identity and secrets** (SSH keys via brittle
side-channels such as synced folders). Authors can already shell out to `op`
inside `run`, but that path has no preflight, no log redaction, and no shared
resolution story across Command entries.

We add a **Secrets substrate**: Config-declared provider + **Secret refs**,
resolved just-in-time into `run` env (and later optional file materialization),
with hard redaction on the **Task event sink**. Delivery is phased:

1. **Phase 0 — SSH agent wedge (shipped):** 1Password SSH agent as the default
   private-git identity path (keys stay in the vault; no `~/.ssh/id_*` copy).
   Root `secrets:`, `secrets::preflight` on doctor/install, recipe
   `onepassword-ssh`.
2. **Phase 1 — Secret refs (not yet):** `from_secret` on `run.env` values,
   OnePassword via the `op` CLI, sink redaction of resolved values.

Sudo stays a **local privilege** concern (existing TUI `pre_authenticate_sudo`,
unattended demotion). We do **not** fetch the OS login password from a vault or
pipe vault secrets into `sudo -S`.

## Decisions

### Opt-in and Phase 0 signal

Configs without a `secrets:` block are unchanged.

Phase 0 is **explicit**, not inferred from SSH `clone` URLs:

```yaml
secrets:
  default_provider: onepassword
  ssh_agent: true
```

`doctor` and pre-`install` then check: `op` on `PATH`, vault session usable,
SSH agent usable when `ssh_agent: true`. Optional host probe (`ssh -T` / git)
when the run set includes private SSH remotes — warning vs error left to
implementation, but failures must name the unlock/agent fix, not opaque git
noise mid-task.

### Phase 1 YAML shape

`run.env` values remain string-compatible and gain a secret form:

```yaml
env:
  PLAIN: "ok"
  GH_TOKEN:
    from_secret: "op://Vault/Item/field"
```

- Resolve **per Command entry** after unlock — not at Config deserialize.
- `validate`: ref shape + provider declared; no unlock required.
- `doctor` / `install`: unlock + resolve smoke (or agent checks) as needed.
- `--dry-run`: show ref identities (`op://…`), never resolved values.
- Nested **Sub-config** documents declare their own `secrets:`; no silent
  cross-document vault ambient state beyond process env / agent.

Optional **write secret to path** (mode `0600`) is deferred until a real tool
demands a file. It is **not** the SSH default (agent is).

### Provider seam

Closed enum / in-tree provider (same spirit as ADR-0006): v1 = OnePassword via
`op`. No plugin loader. A second provider (e.g. Bitwarden) reopens the seam;
until then do not invent a multi-vault matrix.

Env injection alone does **not** justify a new Command kind. A future
file-materialize op must meet ADR-0006 (File ops / modes `run` cannot express
safely).

### Redaction (required from Phase 0)

The **Task event sink** path (TUI and plain) must never emit resolved secret
bytes. Best-effort scrub if a value appears in child stdout/stderr; never dump
full env for Command entries that used `from_secret`. History and debug traces
store ref identities, not values.

### Unattended / schedule

No interactive biometric unlock in cron. Secret-backed work fails closed or is
skipped with a clear reason; do not hang. Service-account / token unlock
(`OP_SERVICE_ACCOUNT_TOKEN` or equivalent) is **deferred** past Phase 1 unless
schedule users block on it — then a follow-up ADR.

### Platform

Phase 0 targets macOS first (primary wipe/restore + 1Password SSH agent story).
Linux follows when agent/`op` parity is confirmed. Windows is Phase 2+ unless
implementation stays trivially shared.

### Sudo (out of success criteria; policy only)

- Keep early TUI sudo cache and unattended demotion.
- Elevated `run` may still receive **non-login** secrets via `from_secret` after
  local sudo auth.
- Forbidden: login password from vault → `sudo -S`.

### `clone` identity

Ambient agent when `secrets.ssh_agent: true`. No first-class
`ssh_identity:` on `clone` in Phase 0/1. Revisit if file-based keys become
necessary for non-agent hosts.

## Non-goals

- Becoming a password manager or syncing vault items.
- age/sops-encrypted Config documents as the primary secrets path.
- Full OS bootstrap (CLT / Homebrew installer engine).
- Multi-provider day-one support.
- Record-mode / capture of existing machine secrets.

## Considered options

- **Document `op read` in bash only:** no preflight, no redaction, easy to leak
  into TUI logs. Rejected as the product answer (remains possible for power
  users).
- **Infer SSH agent from private `clone` URLs:** surprising for public-only
  configs; rejected in favor of explicit `secrets.ssh_agent`.
- **Materialize key files as the default:** recreates the OneDrive pattern with
  a nicer API. Rejected; agent-first.
- **Vault-backed sudo login password:** process-list and log footgun; wrong
  privilege model. Rejected.
- **New `secret:` Command kind for env injection:** YAML sugar; violates
  ADR-0006. Rejected for Phase 1.
- **Service accounts in Phase 1:** needed for headless later; defer to avoid
  blocking the interactive wipe/restore wedge.

## Acceptance (when implementing)

- Serde: string vs `from_secret`; `validate` rejects bad refs.
- Sink redacts known resolved values; mocked `op` integration keeps values out
  of logs.
- Manual wipe: 1Password SSH agent → remote Config → private `clone` without
  copying `~/.ssh/id_*`.
- `doctor` fails closed when locked; passes when unlocked.
- Unattended + secrets does not hang on biometric.

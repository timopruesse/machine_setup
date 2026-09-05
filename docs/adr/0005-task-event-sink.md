# All Task events go through TaskEventSink

The Runner and CommandContext emit every TaskEvent through a **Task event
sink**, not a raw `mpsc` sender. Adapters: **ChannelSink** (production →
TUI/plain) and **NullSink** (Command bench smoke). This isolates FS/Runner
wall-clock from event clone/channel cost and keeps one seam for lifecycle and
CommandOutput alike. Fan-out to multiple simultaneous consumers was rejected as
overbuilt for a single UI consumer.

## Coalesced subprocess output (accepted 2026-09-05 — not yet implemented)

Subprocess line reading may buffer stdout/stderr and emit a
**`CommandOutputBatch`** Task event (multiple lines per flush) instead of one
`CommandOutput` per line. Single-line `CommandOutput` stays for sparse progress
(e.g. FileProgress). Coalescing lives in the engine line reader, not in a new
sink adapter. TUI/plain must expand batches. Bench the ChannelSink path (not
only NullSink) when landing.

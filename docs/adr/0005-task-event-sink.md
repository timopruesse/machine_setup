# All Task events go through TaskEventSink

The Runner and CommandContext emit every TaskEvent through a **Task event
sink**, not a raw `mpsc` sender. Adapters: **ChannelSink** (production →
TUI/plain) and **NullSink** (Command bench smoke). This isolates FS/Runner
wall-clock from event clone/channel cost and keeps one seam for lifecycle and
CommandOutput alike. Fan-out to multiple simultaneous consumers was rejected as
overbuilt for a single UI consumer.

## Bounded ChannelSink (implemented 2026-09-05)

**ChannelSink** uses a bounded `mpsc` channel with capacity **8192**.
`emit` applies backpressure (`try_send`, then on `Full`: `block_in_place` +
`blocking_send` on the multi-thread runtime, plain `blocking_send` outside a
runtime). Direct `blocking_send` on a Tokio worker panics — do not restore
that path. Closed receivers are ignored. **NullSink** is unchanged.

## Coalesced subprocess output (implemented 2026-09-05)

Subprocess line reading may buffer stdout/stderr and emit a
**`CommandOutputBatch`** Task event (multiple lines per flush) instead of one
`CommandOutput` per line. Single-line `CommandOutput` stays for sparse progress
(e.g. FileProgress). Coalescing lives in the engine line reader, not in a new
sink adapter. TUI/plain expand batches. Bench the ChannelSink path (not only
NullSink) when landing.

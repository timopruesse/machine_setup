# All Task events go through TaskEventSink

The Runner and CommandContext emit every TaskEvent through a **Task event
sink**, not a raw `mpsc` sender. Adapters: **ChannelSink** (production →
TUI/plain) and **NullSink** (Command bench smoke). This isolates FS/Runner
wall-clock from event clone/channel cost and keeps one seam for lifecycle and
CommandOutput alike. Fan-out to multiple simultaneous consumers was rejected as
overbuilt for a single UI consumer.

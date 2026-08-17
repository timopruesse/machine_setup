# Exclusive lanes on the Concurrency gate (intra-run package-manager families)

The **Concurrency gate** remains one permit cap (ADR-0003) and also owns
**Exclusive lanes**: at most one Command entry in the run holds a given
package-manager family at a time. We serialize overlapping `run` work that
would contend on the same OS lock (apt/dpkg, brew, …) by waiting on the
lane *before* taking a permit. We do **not** wait on locks held outside this
process (unattended-upgrades) and we do **not** scrape apt errors inside the
`run` Command executor — that recovers after a collision instead of preventing
it, and it is the wrong locality.

Inference lives in the **Command kind catalog** (script text, first matching
family, authors do not declare lanes). The Runner admits; `run` stays unaware.
A lifecycle Task event fires after `CommandStarted` only when the lane is
already held. Dual-family scripts are two Command entries. Nested Sub-config
Runners share the parent's lanes with the parent's gate.

## Considered options

- **Lock-wait inside `run`:** covers external apt, but starts the collision,
  then parses stderr. Rejected as the primary seam.
- **Author-declared exclusive keys:** forget the key and apt still races.
- **One global package-manager lane:** over-serializes brew vs apt.
- **Acquire permit then lane:** waiters occupy slots and starve `copy`/`symlink`.

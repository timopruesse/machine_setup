# Command kind catalog; no plugin loader until a second adapter

Command-entry-kind behavior (validate, `create_executor`, `requires_sudo`, and
related wiring) concentrates in the **Command kind catalog**
(`engine/commands/catalog`), co-located with Command executors. The public
`CommandEntry` enum stays for exhaustiveness; Deserialize may match YAML keys
only to construct the enum. Foreign modules must not `match` on variants for
behavior.

A new kind is justified only when the op needs **Tree materialization**,
**File ops**, **Sub-config** nesting, or Mode semantics `run` cannot express —
not YAML sugar over shell recipes.

We do **not** add a runtime/out-of-tree plugin loader (dylib, inventory, wasm)
while the only adapter is in-tree kinds. One adapter is a hypothetical seam;
reopen when a concrete second adapter exists. Until then, extend via the catalog
and the **Tree-op driver** for tree-shaped kinds.

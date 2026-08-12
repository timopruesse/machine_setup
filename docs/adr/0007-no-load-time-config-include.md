# No load-time Config document include; Sub-config remains the split

Splitting a large Config document across files stays a `machine_setup` Command
entry (nested Sub-config Runner and History). We do **not** add root-level
`include:` / load-time merge into one AppConfig. That would need Task-name
conflict rules and blur authoring split with execution nesting. Reopen only if
“many files, one run, one History” becomes concrete pain; until then Sub-config
is the seam.

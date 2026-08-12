# No in-place Config document Task rewrite

Config document authoring stays **create** (`init`) and **append** (`add task`,
Authoring recipes, wizard). We do **not** structurally edit or delete existing
Task blocks in place (comment-preserving YAML surgery). That is a different
module depth from append-only emit; reopen when a concrete need for `remove` /
upsert on a living hand-tuned file appears. Until then, users edit YAML
directly or replace the file.

- **What a store says about one task is carried into the matter.** A plan in a
  watched task store may keep a block about one of its own tasks in its own
  frontmatter — which slice of the work it belongs to, which customer or
  environment it is about — and ephor now reads it and carries it as `meta`,
  the third reserved `raw` key after `assignees` and `labels`. It reaches the
  four places a matter's other facts already reach: a recipe's `when`, which
  takes a `meta` map where every key must hold and each compares as a string;
  the template vocabulary, where `{meta.<key>}` renders in a brief, a `root`
  and an entry's `branch`; the summons environment, as `EPHOR_META_<KEY>` with
  the fixed `EPHOR_META_KEYS` saying which of those names are this matter's;
  and `--json`, under `raw.meta`. The map is held to identifiers — a scalar on
  one line, at most 1 KiB, under a key a shell will take — with an offending
  key dropped, the rest carried, and the drop reported by the refresh that read
  it, on the error stream and in `refresh --json`'s new `notes`. It is
  read-only inward: the names ephor writes into that same namespace are
  subtracted on the way in, so its own laid plans do not hand its bookkeeping
  back as the store's words. A store that says nothing is a store that said
  nothing, and a `meta` selector refuses it rather than matching. (PR #130)

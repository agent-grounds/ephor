- **The runtime is a module, and `work.runner` is where you point it**
  ([§AR-007-runtime](../../architecture/AR-007-runtime.md#ar-007-runtime-the-runtime-adapter-and-rhei-as-its-shipped-binding),
  manual §4.2.0). Everything runtime-specific now lives in `work/runtime/` —
  the plan language, the runner invocation, the verdict read back — and the
  name of the shipped runtime exists nowhere else in the source. Above that
  module, surfaces name *the binding*: the inbox's banner, `ephor work run`'s
  output and its errors all say whatever `work.runner` says, which is the
  shipped default until you name another. Nothing about writing or reading
  changes when no runtime is installed; only running refuses, naming the
  runner it looked for. The ledger's field was named for the runtime and is
  named for the plan now, with an alias so a ledger written before this still
  reads.

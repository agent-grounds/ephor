# FS-016-browser-opening: a browser action reaches the reader or leaves the address with them

Opening the URL already carried by a reading is presentation, not a new
ability: the command line and the published view do not change
([§REQ-002-parity.1](../requirements/REQ-002-parity.md#1-an-ability-is-a-key-that-reveals-a-fact-or-changes-the-world)). But the browser is outside ephor, so opening it is a seam
with a configured binding, a replaceable shipped default, truthful outcomes,
and a visible floor ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)). The action must never lose the
address or claim that a browser appeared merely because a process started
([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now), [§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

## 1. The browser is a bound opener

`defaults.browser` in `status.json` accepts exactly one of:

- `"xdg-open"`, the binding ephor ships;
- `{ "open": "my-opener {url}" }`, a custom command whose one allowed field is
  `open` and whose template contains `{url}`; or
- `false`, an explicit choice of the copyable-URL floor.

An absent key asks ephor to select automatically. An explicit named or custom
binding wins over every SSH and display signal, and `false` selects the floor
in every environment. A wrong type, an unknown binding name, an unknown object
field, or a custom command without `{url}` is a configuration error; it never
falls through to another opener. Substitution shell-quotes the complete URL as
one argument, including quotes, whitespace, and shell metacharacters
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)).

The shipped product name and command live in `src/seams/browser.rs` and nowhere
else in production source, enforced by the boundary inventory
([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)). Both the TUI's `o` action and its no-thread URL fallback
resolve and invoke this same binding.

## 2. Automatic selection and truthful outcomes

A session is SSH-marked when any of `SSH_CONNECTION`, `SSH_CLIENT`, or
`SSH_TTY` is nonempty after trimming. Absent, empty, and whitespace-only values
do not mark it. Automatic browser selection chooses the shipped binding only
when the session is not SSH-marked and at least one of `DISPLAY` or
`WAYLAND_DISPLAY` is nonempty after trimming; a forwarded or inherited display
never overrides SSH. Browser and window resolution use this one SSH predicate
([§FS-005-dispatch.22](FS-005-dispatch.md#22-a-window-of-the-readers-own-where-one-is-bound)).

The adapter invokes the resolved command through the shared summons executor
in captured mode with a five-second deadline covering exit and capture. The
observed boundary is the configured shell invocation: exit zero says only
`Browser opener exited successfully`, not that a browser appeared or loaded
the URL. Every observed nonzero shell exit, including 126 and 127, says
`Browser opener failed (<code>)`. A shell reporting a missing or non-executable
downstream command therefore reports `failed (127)` or `failed (126)`, just as
a started command choosing those codes does. `Browser opener could not start`
is reserved for positively identified failure to prepare or spawn the
invocation itself; shell creation does not prove downstream execution. Exit
codes, command-controlled stderr and executable preflight checks cannot establish
that distinction. Execution or capture failure after spawn without a usable
exit code says `Browser opener execution failed`, with available diagnostic
detail. Deadline expiry says `Browser opener did not finish within 5 seconds`.
Timeout stops and waits for the opener and cleans up its process group;
capture releases its readers even if a detached descendant still holds a pipe.
A failed configured binding never selects a second one.

The Linux regression proof does not turn scheduler timing into part of this
contract. It first establishes that the direct shell exited while a descendant
retained the streams, then requires the shared deadline to report timeout and
the capture call to return. Process-group cleanup is observed by waiting, under
a separate generous overall bound, until that descendant is absent or a
zombie. A one-shot process-state sample, a short fixed delay, or an unbounded
join proves none of those events. The successful path separately retains both
late stdout and late stderr when the descendant releases them before the
deadline.

Every automatic bypass, explicit `false`, start failure, nonzero exit,
execution failure, or timeout restores the terminal and prints its reason
followed by the complete URL on its own line. It then waits for Enter before
the TUI redraws. The URL is not shortened to the screen or status-line width,
so the terminal's scrollback
retains it byte for byte. Automatic SSH bypass says `Browser opener bypassed:
SSH session`; automatic headless bypass says `Browser opener bypassed: no
graphical display`; and `false` says `Browser opener disabled by configuration`.

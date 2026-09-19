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
in captured mode with a five-second deadline. Exit zero says only `Browser
opener exited successfully`; it does not say that a browser appeared or loaded
the URL. A command that cannot start says `Browser opener could not start`; a
nonzero exit says `Browser opener failed (<code>)`; and a command still running
at the deadline says `Browser opener did not finish within 5 seconds`. Timeout
stops and waits for the opener, and a failed configured binding never selects a
second one.

Every automatic bypass, explicit `false`, start failure, nonzero exit, or
timeout restores the terminal and prints its reason followed by the complete
URL on its own line. It then waits for Enter before the TUI redraws. The URL is
not shortened to the screen or status-line width, so the terminal's scrollback
retains it byte for byte. Automatic SSH bypass says `Browser opener bypassed:
SSH session`; automatic headless bypass says `Browser opener bypassed: no
graphical display`; and `false` says `Browser opener disabled by configuration`.

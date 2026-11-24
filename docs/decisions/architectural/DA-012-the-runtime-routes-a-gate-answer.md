# DA-012-the-runtime-routes-a-gate-answer: the runtime turns a gate's answers into its move, and ephor only sends them

**Status:** Accepted
**Date:** 2026-11-24

A gate the workflow prepared asks its questions and takes typed answers
([§FS-005-dispatch.9.3](../../functional-spec/FS-005-dispatch.md#93-a-prepared-gate-is-answered-in-its-own-form)). Someone has to turn those answers into a move — this
*yes* means the ticket goes on, that choice sends it back for revision. Two
hands could do it: ephor's, since ephor draws the form and holds the answers
the moment they are given, or the runtime's, which declares the gate, its
questions and its states. This record fixes the second and names what the
first would have cost.

## 1. The decision

Ephor sends the answers and the runtime decides the move. The runtime module
([§AR-007-runtime.1](../../architecture/AR-007-runtime.md#1-what-the-module-owns)) asks the bound runner for the gate's brief and hands it up
as data: the questions, the routes and destinations the runtime declares, and
a digest the runtime computed over what it asked. The screen and the command
line build one answer document — the runtime's own version, that digest copied
unchanged, the answers by question id — and the module passes it to the
runner's answer verb, first as a dry run and then for real. What the runner
answers is what the reader is told: where the ticket goes, by which route,
where the answer is recorded, or which checks refused it. Nothing above the
module looks at a route's condition, picks a destination from an answer, or
hashes anything; a route is shown as the runtime wrote it, and a field the
runtime left unknown is shown as unknown.

Whether the runner can do any of this is asked of the runner itself, once per
process and per runner, and where it cannot, every surface is what it was
([§AR-007-runtime.3](../../architecture/AR-007-runtime.md#3-degrade), [§REQ-001-boundary.1](../../requirements/REQ-001-boundary.md#1-the-anatomy)).

## 2. The rejected alternative

Ephor maps the answers. It draws the form, so it could evaluate the routes as
the reader fills it in, show the destination before anything is sent, and spare
the dry run's process start on every submit. Two things rule it out. Ephor may
not compute an answer of its own: a surface renders what the API returns, and
the API passes on what the binding says ([§AR-009-surfaces.1](../../architecture/AR-009-surfaces.md#1-the-api-is-readings-and-moves)). And ephor may not
know what a workflow's states mean — approval, revision and dismissal are names
the workflow gives its states, never names ephor assumes
([§FS-005-dispatch.9.1](../../functional-spec/FS-005-dispatch.md#91-a-human-gate-is-reviewed-and-moved-where-its-matter-is-read)). A mapping in ephor would put the meaning of a workflow
on the wrong side of the binding, and every other surface the runtime has —
its own dashboard, its own run screen — would need a second copy of the same
mapping, which would drift from the first the day a route grammar grew a term.

## 3. The cost

Every submit costs two runner invocations, the dry run and the answer, where a
mapping in ephor would cost one. Accepted: a person is answering, the runner
is a local process, and the dry run is the confirmation the reader reads before
anything moves, so it is not overhead but the screen. The form also cannot say
where the ticket will go until the reader asks; it says how many questions are
answered and which required ones are not, and leaves the destination to the
runtime's verdict. And the move is only as capable as the runner's verb, as a
cancel is ([§DA-005-cancel-is-the-runtimes-move](DA-005-cancel-is-the-runtimes-move.md#da-005-cancel-is-the-runtimes-move-cancelling-a-ticket-asks-the-runtime-and-never-rewrites-the-state-line)): what the runner refuses, ephor
reports and does not work around.

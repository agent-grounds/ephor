#!/usr/bin/env bash
# Hand ephor the chat conversations a listener has heard, as a forge answering
# `messages` (§FS-001-forge-interface.1).
#
# This is the chat seam's worked example, and it ships beside ephor rather than
# inside it on purpose (§REQ-001-boundary.5): how a chat network is listened to
# is volatile — a linked device, a bot token, a bridge — and a literal compiled
# into ephor would go stale in ephor's release cycle rather than in yours. What
# ephor fixes is the contract below; the listener is yours.
#
# This queue-only example deliberately leaves reply_reconciliation undeclared
# (§FS-001-forge-interface.1). Queue admission is not known remote acceptance:
# a declaring gateway must durably look up repeats before descriptor freshness,
# return prior acceptance without another delivery, and refuse changed payload.
# Uncertain sends through this example are held for a checked local resolution
# (§FS-005-dispatch.13).
#
# THE SPLIT. A chat network is heard by something that stays connected, and
# ephor does not stay connected to anything: it asks, on `refresh`, and the
# answer has to be ready. So an always-on LISTENER, outside ephor, keeps a
# record of what it heard in a SPOOL directory, and this script reads that
# record when ephor asks. ephor never sees the listener, the network, or the
# account; it sees conversations.
#
# Link it as a forge and declare it ONCE for the site, so its conversations are
# placed by the rooms projects claim rather than by where it was declared
# (§FS-001-forge-interface.9, §FS-008-attribution.1):
#
#     ln -s …/config/chat-gateway.example.sh ~/.local/bin/ephor-forge-chatgw
#
#     status.json     { "sources": [ { "provider": "chatgw",
#                                      "spool": "~/.local/state/chat",
#                                      "account": "you",
#                                      "max_age_seconds": 300 } ],
#                       "projects": { "widget": { "providers": [] } } }
#
#     workspaces.json { "id": "widget", "rooms": ["whatsapp/acme#120363@g.us"], … }
#
# Everything in the source block is this script's own and is handed to it
# verbatim; ephor reads none of it (§FS-001-forge-interface.4):
#
#     spool             where the listener keeps its record. A leading `~/` is
#                       your home directory.
#     account           the author name your own messages carry in the spool.
#                       Only this side knows how the network names you, so this
#                       side says which messages are `mine` and ephor works out
#                       the rest (§FS-001-forge-interface.3).
#     max_age_seconds   how long ago the listener may last have confirmed it was
#                       hearing the network before this script stops vouching
#                       for the spool. Default 300.
#
# THE SPOOL is the listener's to write and this script's to read:
#
#     listener.json         written when the listener is paired with an account,
#                           and kept current while it runs:
#                             { "link": "up",
#                               "observed_at": "2026-10-01T09:14:00Z" }
#                           `link` is "down" while it cannot reach the network.
#                           `observed_at` is the last time the network itself
#                           confirmed the listener was hearing it — a sync, an
#                           acknowledgement — whether or not anybody spoke.
#     conversations/*.json  one conversation per file, in the shape `messages`
#                           answers below, without `mine`.
#     outbox.jsonl          replies this script queued, one JSON object per
#                           line, { "target": …, "text": … }, for the listener
#                           to send and then record in the conversation as
#                           yours.
#
# THE PROTOCOL is the forge's: `ephor-forge-chatgw <subcommand>`, the request as
# JSON on stdin, the answer as JSON on stdout, a diagnosis on stderr
# (§FS-001-forge-interface.2, `ephor schema forge`).
#
#     capabilities   { "messages": true, "replies": true }
#     messages       every conversation in the spool, whole:
#                      [ { "id": "whatsapp/acme#120363@g.us",
#                          "title": "Widget rollout: can we ship Friday?",
#                          "updated_at": "2026-10-01T09:12:00Z",
#                          "room": "whatsapp/acme#120363@g.us",
#                          "reasons": ["mentioned"],
#                          "threads": [ {
#                            "messages": [ { "author": "dana", "mine": false,
#                                            "text": "Can we ship the widget on Friday?",
#                                            "when": "2026-10-01T09:12:00Z" } ],
#                            "reply": { "chat": "120363@g.us" } } ] } ]
#                    `id` is stable across refreshes: ephor keys the row
#                    `chatgw:<id>` and tracks what you have read by it. `room`
#                    is absent for a direct conversation. Whether a
#                    conversation waits on you is ephor's to decide from the
#                    messages, never this script's to say.
#                    A message may name the files sent with it, and this
#                    script passes the list on as the listener wrote it:
#                      "attachments": [ { "name": "IMG_2041.jpg",
#                                         "media_type": "image/jpeg",
#                                         "size": 1843211,
#                                         "id": "att:dana:2041" } ]
#                    `name` is required; the rest only where the network
#                    says. Leave the list out where the listener does not
#                    record files, and write `[]` where it saw none. ephor
#                    names the files and never fetches one; `id` is the
#                    listener's own, kept to be handed back
#                    (§FS-001-forge-interface.1).
#     reply          queue `.text` for the thread whose `reply` descriptor is
#                    `.target`, handed back exactly as `messages` gave it.
#
# A conversation reported here is not also reported as a notice: a notice
# carries the gateway's own claim that something needs you, and ephor would
# keep it.
#
# HOW IT FAILS. An empty answer means nothing is waiting, so it is only given
# when this side can show it is still hearing the network
# (§FS-001-forge-interface.6). Each other case is a non-zero exit and one line
# on stderr saying what to do about it:
#
#     never paired            no listener.json: pair the listener first
#     link down               "connection refused": ephor shows the gateway as
#                             unreachable, and its last rows as stale
#     cannot vouch            the listener's last confirmed observation is older
#                             than max_age_seconds, or it never recorded one. A
#                             listener that is still running proves nothing,
#                             and neither does the age of the newest message:
#                             a quiet room is quiet, a deaf listener is not.
#     a quiet room            not a failure: the listener is hearing the
#                             network and nobody spoke, so the answer is `[]`
#
# Copy this, point it at your listener's spool, and keep the shape.
set -uo pipefail

subcommand=${1:-}
request=$(cat)

fail() {
  echo "$*" >&2
  exit 1
}

case "$subcommand" in
  capabilities)
    # Asked before any configuration is read, so it never depends on one.
    printf '{"messages":true,"replies":true}\n'
    exit 0
    ;;
  messages | reply) ;;
  *)
    echo "unsupported: $subcommand" >&2
    exit 64
    ;;
esac

spool=$(jq -r '.config.spool // empty' <<<"$request")
[ -n "$spool" ] || fail "no spool is configured: set \"spool\" on this source to the listener's directory"
case "$spool" in \~/*) spool="$HOME/${spool#\~/}" ;; esac
account=$(jq -r '.config.account // empty' <<<"$request")
max_age=$(jq -r '.config.max_age_seconds // 300' <<<"$request")

# ── is anybody listening ─────────────────────────────────────────────────────
listener="$spool/listener.json"
[ -f "$listener" ] ||
  fail "never paired: no listener has recorded an account in $spool — pair the listener with your account, then refresh"

# How long ago the network last confirmed the listener was hearing it, and
# whether that is recent enough to answer for. A timestamp the listener never
# wrote is no evidence at all.
vouch() {
  if [ "$(jq -r '.link // "up"' "$listener")" = "down" ]; then
    fail "connection refused: the listener cannot reach the chat network (last heard it at $(jq -r '.observed_at // "never"' "$listener"))"
  fi
  local age
  age=$(jq -r '(.observed_at // empty) | (now - fromdateiso8601) | floor' "$listener" 2>/dev/null)
  [ -n "$age" ] ||
    fail "cannot vouch for the spool: the listener has recorded no observation of the chat network — check that it is running and connected"
  [ "$age" -le "$max_age" ] ||
    fail "cannot vouch for the spool: the listener last confirmed it was hearing the chat network ${age}s ago, more than max_age_seconds ($max_age) allows — check that it is running and connected"
}

# ── answer ───────────────────────────────────────────────────────────────────
case "$subcommand" in
  messages)
    vouch
    shopt -s nullglob
    conversations=("$spool"/conversations/*.json)
    if [ ${#conversations[@]} -eq 0 ]; then
      # Heard, and nobody spoke: an answer, and an empty one.
      printf '[]\n'
      exit 0
    fi
    jq -s --arg me "$account" '
      map(.threads |= map(.messages |= map(.mine = (.author == $me))))
    ' "${conversations[@]}"
    ;;
  reply)
    # Queued for the listener to send when it can, so only a gateway nobody
    # paired refuses one.
    jq -c '{ target: .target, text: .text }' <<<"$request" >>"$spool/outbox.jsonl" ||
      fail "could not queue the reply in $spool/outbox.jsonl"
    printf '{}\n'
    ;;
esac

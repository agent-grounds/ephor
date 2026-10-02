- **A reply is sent only to a forge that said it sends replies.** A thread's
  `reply` descriptor alone used to be enough. A forge must now also declare
  `"replies": true`, and one that has not is refused with *`<source>` does not
  send replies*. A dry run makes the same check and names the source it would
  send through. (PR #155)

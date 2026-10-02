- **An issue nobody has taken can count as work waiting**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities),
  [§FS-003-feed-categories.4](../../functional-spec/FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)).
  Whether an issue awaits you was read off the conversation — last word, or a
  reaction, or a forge-tracked task — and the conversation is exactly what
  cannot say this: an issue somebody filed and nobody picked up has its
  author's word last, so the rule that serves every other case reports it as
  answered. A backlog of ten open issues on your own project read as ten
  finished things. It is the same shape as a review asked for and never given:
  being waited on leaves no message behind. So assignment crosses the interface
  as a fact — `Issue` reports whether anyone has taken it, and an
  implementation with no notion of assignment omits it and is never counted as
  unclaimed — and an unclaimed issue is a fourth form of pending beside the
  three above. Whether it applies is the source's to say (`unclaimed: true`,
  off by default), because *unclaimed* only means *yours* where you are
  answerable for the backlog: on a project you run it is the whole point, and
  among issues you once commented on somewhere it would turn every stranger's
  open bug into your work.

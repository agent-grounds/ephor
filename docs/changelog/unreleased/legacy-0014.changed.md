- **The changelog gate asks for a bullet, not for a number nobody can know yet**
  ([§FS-002-release.1](../../functional-spec/FS-002-release.md#1-changelog),
  [§FS-002-release.6](../../functional-spec/FS-002-release.md#6-the-changelog-gate-runs-before-the-pull-request-exists)).
  A pull request is now asked for a new or changed bullet under `## Unreleased`
  relative to its own base, rather than for `PR #<n>`: the number does not exist
  when the bullet is written, so an outside contributor's first CI run used to
  fail on something they could not have obeyed. A number that *is* written must
  still be that pull request's own, and `PR #TBD` is accepted. Two places it is
  felt. The `changelog-pr-entry` pre-push hook no longer stands down when there
  is no pull request yet — it refuses a push that adds no bullet and names
  `SKIP=changelog-pr-entry git push`, the one hook to skip, so a forgotten
  bullet is caught at the keyboard rather than on a stranger's first CI run;
  where no base can be resolved at all it falls back to requiring that
  `## Unreleased` carries a bullet and says which check it ran. And the release
  stamps the numbers, blaming each `## Unreleased` bullet and writing `PR #<n>`
  where every line of it resolves to the same single pull request
  ([§FS-002-release.2](../../functional-spec/FS-002-release.md#2-cutting-a-release)).
  Stamping never fails a release, so a released section may keep a bullet
  carrying no number at all. (PR #140)

- **Nothing on Windows was ever found on `PATH`, and no bound script there was
  ever recognized as a path.** `command_exists` looked for the bare name, but
  an executable on `PATH` there is `sh.exe` rather than `sh` — so every rung
  that asks "is this runner installed" answered no on a machine where the
  runner was sitting right there, and the loop refused to run. It honours
  `PATHEXT` now. Separately, `missing_binding` decided what looked like a path
  by testing for a leading slash, so `C:\tools\check.bat` read as a bare
  command: a script that had been deleted was handed to the shell to fail as
  "command not found" instead of being refused at invocation by name
  ([§AR-005-capabilities.3](../../architecture/AR-005-capabilities.md#3-the-table-is-honest-about-time)). What counts as absolute is the platform's answer
  now, which is the same answer as before everywhere else. Both were found by
  the Windows leg of CI, which had never compiled far enough to run a test.

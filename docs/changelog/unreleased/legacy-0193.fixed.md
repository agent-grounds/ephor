- **The test tree could not compile on Windows, and failed on macOS.** Two
  helpers imported `std::os::unix::fs::PermissionsExt` unconditionally, so
  every integration and scenario binary failed to build on `windows-latest` —
  including the ones that summon no shell at all. The exec bit is set under
  `#[cfg(unix)]` now, the shape the seam's own tests already used. On macOS the
  temporary directories are `/var/folders/…` and really `/private/var/…`, and a
  summoned shell prints the second spelling as its `$PWD`: three tests compared
  two spellings of one directory and failed there and nowhere else. The world
  is built inside a base directory the operating system has already resolved,
  so both sides are one spelling.

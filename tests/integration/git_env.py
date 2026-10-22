"""Git's repository variables, kept out of every subprocess the Python suite starts. §REQ-003-test-isolation.3.4

Not a test module - `unittest discover -p 'test_*.py'` does not collect it. The
pre-commit hook runs this suite, and git exports `GIT_DIR`, `GIT_INDEX_FILE` and
their companions to every hook. A fixture's `git init` and `git remote add`, or a
release script's own `git`, would act on the repository being committed to
rather than the one the fixture made (§REQ-003-test-isolation.3.3). Every fixture
builds the environment of what it starts from `environ` below, so the removal
is written once.
"""

import os


def environ(**deliberate: str) -> dict:
    """This process's environment with every `GIT_*` taken off, then what the caller means to set.

    The prefix rather than a list, so `GIT_CONFIG_PARAMETERS`, which carries an
    outer `git -c` down the same inheritance, goes too (§REQ-003-test-isolation.3.1).
    What the caller passes comes after the removal and is kept, the way a fixture
    authors its commits with `GIT_AUTHOR_NAME` (§REQ-003-test-isolation.3.2).
    """
    return {key: value for key, value in os.environ.items() if not key.startswith("GIT_")} | deliberate

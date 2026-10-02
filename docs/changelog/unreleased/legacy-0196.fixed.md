- **A finished status line appeared under both Status and Recent**
  ([§FS-003-feed-categories.1](../../functional-spec/FS-003-feed-categories.md#1-the-categories)). Every
  interactive section but Recent's excluded finished work; Status did not, so
  a project answering `"status": "done"` was double counted in exactly the
  pile the categories exist to make readable. The plain renderer had it right.

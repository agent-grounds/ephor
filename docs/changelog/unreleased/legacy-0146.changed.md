- Work runs **from the checkout it is about**. The runtime finds the agent's
  working directory by looking for the git repository enclosing the plan; a
  multi-repo workspace holds several and may be none, and the fallback is then
  whatever directory the runtime was started in — which `ephor work run` had
  left as wherever it was typed. The checkout is now recorded in the ledger at
  dispatch and both `work run` and the interface's `R` run there, so the root
  of a multi-repo project is the directory its repositories sit in whether or
  not that directory happens to be a repository.

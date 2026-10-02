- **Recipes and workflow entries can place their own work root**
  ([§FS-005-dispatch.1](../../functional-spec/FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for),
  [§FS-005-dispatch.6.1](../../functional-spec/FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)).
  Optional `root` templates on recipes and flat workflow entries override
  project, organization and site placement after branch resolution, letting
  one project keep issue fixes in minted checkouts and maintenance work at its
  project root. Every committed placement stays visible with its root,
  checkout and branch. (PR #101)

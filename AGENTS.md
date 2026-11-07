# OpenNMR project memory

## Pull requests

Before opening any PR, read this section and check the final title, body,
captions, linked explanations, and images against it. Repeat the check when
updating reviewer material. Perform this review internally; do not paste a
checklist into the PR.

- Use concise, natural language. Lead with what changed and why it matters.
  Assume the reviewer may be unfamiliar with NMR; explain essential terms with
  a short, concrete example. Avoid unnecessary implementation details, hashes,
  identifiers, validation inventories, and exhaustive checklists.
- Include a brief line describing how the change was checked. State material
  limitations plainly, without claiming more than the evidence supports.
- Where useful, include real before/after figures, screenshots, or measured
  percentage comparisons. For UI changes, prefer a runnable local preview with
  the implemented components and sample data; label the preview's fidelity.
- Inspect every image before publication. Keep cursors, pointer highlights,
  and automation overlays out of screenshots. Prefer direct plot exports for
  scientific figures when practical; preserve the actual signal and data.
- Label comparisons clearly: distinguish previous-versus-updated versions from
  deliberately misaligned-versus-corrected examples in the same version.
  Explain what percentages measure and which direction is better. Consistency
  measurements must not be described as sample purity or absolute accuracy.
- Do not mention competitors in new project or reviewer-facing material.
- Do not include AI agent, model, skill, or internal review-workflow attribution
  in PRs or their supporting reviewer material.
- Use `bugfix/<description>` for fixes and `feature/<description>` for features.
  Never use `agent/`; use other prefixes only when explicitly requested.
- Keep commit messages concise and natural. The user's personal CLI is fish.

PR1's approved presentation is the reference: a short plain-language explanation,
a clearly defined comparison graph, and cursor-free before/after plot exports.

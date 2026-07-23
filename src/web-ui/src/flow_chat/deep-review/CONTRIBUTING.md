# DeepReview Frontend Contributions

Use this guide for frontend DeepReview changes in `src/web-ui`.

- Keep launch, manifest building, queue UI, report rendering, and remediation
  actions aligned with backend contracts.
- Keep review-team types/defaults, event mapping, action-bar state, report
  components, and locale strings in sync.
- Keep manifest planning metadata-only; never include source text or full diffs.
- Use infrastructure APIs instead of direct Tauri calls from components.
- Verify with the narrowest matching web checks for the files touched.

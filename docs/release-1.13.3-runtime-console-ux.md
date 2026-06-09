# Gateway Switch 1.13.3 Release Notes

Version: v1.13.3
Date: 2026-06-08
Repository: https://github.com/gcristiano0624-bot/gateway-switch

## Summary

Gateway Switch 1.13.3 reorganizes the user experience after feedback that the navigation was too noisy versus cc-switch. The previous "route CRUD only inside Route Builder" rule is reverted, and the sidebar is condensed into a smaller, role-oriented set of groups.

## Highlights

- Sidebar reorganized into 5 groups: Overview / Apps / Setup / Diagnostics / Advanced. All labels are now bilingual.
- Claude Desktop / Claude Code / Codex pages embed inline Add / Edit / Delete for their own routes. No more cross-page jumps to edit a route you can already see.
- Route Builder kept as an Advanced multi-target editor; per-app pages link to it as "Advanced Route Builder".
- Dashboard "Next Actions" card surfaces up to 5 click-to-fix CTAs derived from current state (no provider, no route, gateway stopped, app not bound, diagnostics critical, recent failure).
- One-step First-run setup card on the Dashboard when no provider is configured, with the embedded provider wizard.
- ProviderSetupWizard now bilingual (accepts `t` and `showHeader` props).
- Hard-coded English/Chinese strings removed from Dashboard, Provider Console and Diagnostics page headers; 30+ new bilingual entries added.

## Verification

- `cargo test` — 75 passed, 0 failed, 3 ignored
- `pnpm build` — passed
- `pnpm tauri build` — DMG generated and validated

## Artifact

- DMG: `Gateway Switch_1.13.3_aarch64.dmg`
- Tag: `v1.13.3`

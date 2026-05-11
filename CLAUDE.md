# Gateway Switch Agent Notes

Gateway Switch is the active Tauri desktop app in this folder.

## Commands

- Install dependencies: `pnpm install`
- Frontend build: `pnpm build`
- Tauri dev app: `pnpm tauri dev`
- Tauri bundle: `pnpm tauri build`
- Rust tests: `cd src-tauri && cargo test`

When working from the parent `ClaudeGateway` folder, run commands with an explicit project directory:

- `pnpm --dir gateway-switch build`
- `pnpm --dir gateway-switch tauri dev`
- `pnpm --dir gateway-switch tauri build`

## Preview

The browser Vite preview runs outside Tauri, so the UI uses mock data there. Real binding, gateway, and config operations require `pnpm tauri dev`.

# Gateway Switch 1.7.0 Runtime Validation

Date: 2026-05-18

## Goal

Validate that the packaged `Gateway Switch.app` can launch locally and that the new Cold Start Doctor data path works in a real runtime.

## Packaged App Used

- `src-tauri/target/release/bundle/macos/Gateway Switch.app`

## Launch Method

Executed the packaged macOS app binary directly:

```bash
"src-tauri/target/release/bundle/macos/Gateway Switch.app/Contents/MacOS/gateway-switch"
```

## Observed Runtime Evidence

The packaged app started successfully and remained running.

Observed process evidence:

- packaged binary process exists under the local bundle path
- app process is active while validation command is running

Observed coldstart backend log evidence from the running packaged app:

```text
[coldstart][system][ok] Environment discovery: Loaded local app state, settings path, database path, and binding targets
[coldstart][gateway][ok] Provider and route inventory: 0 providers, 0 Claude routes, 0 Codex routes
[coldstart][capability][Claude][ok] Claude Desktop config: ... managed=true, base_url=http://127.0.0.1:3456
[coldstart][capability][Claude Code][ok] Claude Code config: ... managed=true, base_url=https://token-plan-sgp.xiaomimimo.com/anthropic
[coldstart][capability][Codex][ok] Codex config: ... managed=true, base_url=http://127.0.0.1:3457/v1
[coldstart][capability][Claude][ok] Claude health endpoint: 200 OK ...
[coldstart][capability][Codex][ok] Codex health endpoint: 200 OK ...
[coldstart][system][ok] Generate coldstart report: Compiled UI report, safe-fix results, manual remediation list, and security notes
```

## What This Confirms

- The packaged `Gateway Switch.app` launches successfully.
- The packaged app initializes the cold-start inspection pipeline in a real runtime.
- The frontend-to-Tauri coldstart status fetch path is working, because the packaged app is repeatedly invoking coldstart inspection logic and producing structured logs.
- Local Claude and Codex health endpoints are reachable from the packaged app runtime.
- The new coldstart logging format is active in the packaged build.

## Runtime Limitation Encountered

A full macOS UI click-through test of the `Run Check & Safe Fixes` button could not be completed automatically in this environment because UI scripting through `System Events` is blocked by macOS accessibility permissions:

```text
execution error: “System Events”遇到一个错误：发生权限违例。 (-10004)
```

This means:

- the packaged app itself did run
- the coldstart inspection path did run
- but an automated button-click test for the repair action could not be completed from this sandbox without granting Accessibility control

## Additional Runtime Observation

Because local ports `3456` and `3457` were already occupied by existing gateway processes, the packaged app also produced expected status warnings such as:

- `Address already in use (os error 48)`

This is acceptable for the validation context and shows that the packaged app is correctly detecting real runtime port conflicts instead of failing silently.

## Conclusion

Current validation result:

- `Cold Start Doctor` status-check path: validated in packaged runtime
- `Cold Start Doctor` repair-button click path: not fully automated in this sandbox due macOS Accessibility restriction

## Recommended Next Manual Check

If you want a final end-to-end UI confirmation on this Mac, do this once manually after granting Accessibility permission to the controlling terminal/IDE:

1. Open `Gateway Switch.app`
2. Enter `Cold Start`
3. Click `Run Check & Safe Fixes`
4. Confirm a report path appears in the page
5. Confirm a report file is generated under the app backup `coldstart/` directory

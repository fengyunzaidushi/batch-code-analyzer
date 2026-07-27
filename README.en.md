# Batch Code Analyzer

[简体中文](README.md) · English

Batch Code Analyzer is a local-first desktop tool for batch AI analysis of code files across Windows, macOS, and Linux. It scans registered local repositories, filters real project files safely, sends one request per file to services compatible with the OpenAI Responses API, and stores traceable Markdown results, run statistics, and request history. It never modifies source code.

## Core capabilities

- Register and switch between multiple local code repositories.
- Filter files using `.gitignore`, file type, size limits, and security rules.
- Configure project or per-file prompts and models.
- Analyze files in batches through an OpenAI Responses API-compatible provider.
- Review Markdown results, token usage, latency, errors, retries, and primary/backup profile switching.
- Save every run into an isolated output directory for reproducibility and recovery.
- Build project context from repository documentation such as `README` and `AGENTS.md`.
- Keep API secrets in the operating system's secure credential store instead of plain configuration files.

## Technology stack

- Desktop shell: Tauri 2
- Frontend: React, Vite, TypeScript, TanStack Query/Table/Virtual, Zustand
- Core backend: Rust, Tokio, reqwest
- Database: SQLite with sqlx
- Secure storage: OS Keychain / Credential Manager / Secret Service, with Stronghold as an explicit fallback
- Release automation: GitHub Actions and Tauri Updater

## Downloads and releases

Pushing a version tag makes GitHub Actions build and upload installers for all three platforms to a release:

- Windows: `.msi` and `.exe` (NSIS)
- macOS: Intel and Apple Silicon `.dmg`
- Linux: `.AppImage` and `.deb`

```bash
git tag v0.1.0
git push origin v0.1.0
```

Code signing is not configured in the current release workflow. The first launch may require a security confirmation on macOS or Windows. Developers can also run `pnpm tauri:build` to build locally.

## Documentation

| Document | Purpose |
| --- | --- |
| `AGENTS.md` | Repository rules for AI agents and developers |
| `docs/prd.md` | Product requirements, scope, defaults, and acceptance criteria |
| `docs/architecture.md` | Architecture, module boundaries, data design, and release plan |
| `docs/contracts/ipc-contract.md` | Tauri commands, events, and DTO contracts |
| `docs/contracts/database-schema.md` | SQLite tables, indexes, transactions, and migrations |
| `docs/contracts/task-state-machine.md` | Run, Task, and Attempt states and transitions |
| `docs/contracts/error-codes.md` | Stable error structure and error code naming |
| `docs/decisions/` | Approved architecture decisions |
| `tasks/` | Task briefs and implementation templates |

## Current status

The repository contains the initial pnpm monorepo and Cargo workspace, including `apps/desktop` (Tauri 2 + React + Vite) and the foundational `crates/*` modules.

## Local development (macOS)

### Prerequisites

```bash
xcode-select --install
corepack enable
corepack prepare pnpm@11.9.0 --activate
rustup toolchain install stable
```

Node.js 20 or later is recommended (22 LTS preferred).

### Install dependencies

```bash
pnpm install
```

### Start the app

```bash
# Start the Tauri desktop app (recommended)
pnpm tauri:dev

# Start only the frontend development server
pnpm dev
```

### Optional health checks

```bash
cargo check --workspace
pnpm typecheck
```

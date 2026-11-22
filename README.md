# agent-orchestrator

`skillctl` is a small command-line tool for [Agent Skills](https://agentskills.io): skill folders, each with a `SKILL.md` and an optional `references/` folder. It loads and validates skills, routes tasks to model tiers, exports skills to other agents' instruction formats, and installs skill folders where each agent looks for them.

It is the first step of the **runtime layer** described in the agent-skills repo's `ROADMAP.md`. The skills hold the knowledge. The runtime will run them across models, continuously. This MVP makes no network calls and does not use an async runtime.

## Install

```sh
cargo install --path .
```

Built and tested with Rust 1.97.

## Commands

Every command reads skills from `--skills-dir` (default `./skills`).

```sh
# Name + first sentence of each description
skillctl list --skills-dir ../agent-skills/skills

# Print a skill's SKILL.md
skillctl show agent-orchestra

# Same checks as agent-skills' scripts/validate.py; exit code 1 on errors
skillctl validate

# Routing matrix from agent-orchestra (each factor 1–3)
skillctl route --complexity 2 --risk 2 --context 2 --capability 1
# tier: T2
# reason: all factors ≤ 2, sum 7 (7–8 → T2)
skillctl route --complexity 1 --risk 3 --context 1 --capability 1 --sensitive
# tier: T1
# reason: risk = 3 (any factor 3 → T1)
# providers: local/compliant providers only

# Export to another tool's format
skillctl export --format cursor --out .

# Copy skill folders into an agent's native skills directory
skillctl install --target claude                 # ./.claude/skills/<name>/
skillctl install --target agents --scope user    # ~/.agents/skills/<name>/
skillctl install --target cursor --dest ../app agent-behavior memory
```

### Validation

`validate` checks that SKILL.md exists and has frontmatter that parses as YAML. The name must be 1–64 characters of `[a-z0-9-]`, with no leading or trailing hyphen and no `--`, and must match the folder name. The description must be non-empty and at most 1024 characters. SKILL.md must be 500 lines or fewer, and every `` `references/...` `` file it mentions must exist. Two repo-level checks run only when the skills folder's parent directory has them: `evals/<name>.md` must exist, and `README.md` must list `skills/<name>/SKILL.md`.

### Routing

The rule from `skills/agent-orchestra/references/routing-matrix.md`: if any factor is 3, the tier is T1. Otherwise a sum of 6 or less gives T3, and a sum of 7–8 gives T2. `--sensitive` restricts the task to local or compliant providers, whatever the tier.

### Export formats

| `--format` | Output (under `--out`) |
|---|---|
| `agents-md` | `AGENTS.md`: short intro and an index of skills with paths to each SKILL.md, relative to `--out` |
| `gemini` | `GEMINI.md`, with the same content as `agents-md` |
| `bundle` | `SKILLS.md`: table of contents plus every SKILL.md body, for system prompts or custom instructions |
| `aider` | `CONVENTIONS.md`, with the same content as `bundle` |
| `cursor` | `.cursor/rules/<name>.mdc` with `description`, empty `globs` and `alwaysApply: false` (`true` for `agent-behavior`) |
| `copilot` | `.github/copilot-instructions.md`: index of skills (pointing to `.github/skills/`) + `agent-behavior` body. Copilot loads skills natively, so install them with `skillctl install --target copilot` instead of per-file instructions that would load every skill on every request |
| `windsurf` | `.windsurf/rules/<name>.md` with `trigger: model_decision` (`always_on` for `agent-behavior`) and `description` |
| `continue` | `.continue/rules/<name>.md` with `name`, `description` and `alwaysApply` |
| `claude-zip` | `<name>.zip` per skill, with the skill folder at the archive root, for upload to Claude.ai |

Rule formats (`cursor`, `windsurf`, `continue`) contain only the SKILL.md body, with a note pointing to `.agents/skills/<name>/references/`; run `skillctl install --target agents` so those reference files exist. `AGENTS.md`/`GEMINI.md` index entries point to the same install location.

Gemini CLI can also read `AGENTS.md` directly if you add `{"context":{"fileName":["AGENTS.md","GEMINI.md"]}}` to its `settings.json`.

### Install targets

| `--target` | `--scope project` (under `--dest`, default `.`) | `--scope user` (under `--dest`, default `$HOME`) |
|---|---|---|
| `agents`, `codex` | `.agents/skills/` (read by Codex, Cursor, Copilot, Gemini CLI, Windsurf) | `~/.agents/skills/` |
| `claude` | `.claude/skills/` | `~/.claude/skills/` |
| `cline` | `.cline/skills/` | `~/.cline/skills/` |
| `cursor` | `.cursor/skills/` | `~/.cursor/skills/` |
| `copilot` | `.github/skills/` | `~/.copilot/skills/` |
| `gemini` | `.gemini/skills/` | `~/.gemini/skills/` |
| `windsurf` | `.windsurf/skills/` | `~/.agents/skills/` |

If a skill folder already exists at the destination, `install` refuses to continue. Pass `--force` to replace the folder.

## Library

The crate is also a library (`agent_orchestrator`) with these modules for later runtime work:

- `skill`: parse frontmatter, load and check skills.
- `validate`: the validate.py checks.
- `routing`: the tier matrix (`Tier`, `Factors`, `route`).
- `contract`: `TaskContract` and `Verdict`, round-tripped through YAML. Fields follow agent-orchestra's `references/contracts.md`.
- `runlog`: the append-only run log line format from `contracts.md` (`Entry`, `append`, `read`).
- `export`, `install`: the formats and targets above.

## Development

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Integration tests run the binary against temporary fixtures. They also run against the real agent-skills repo, read-only, when it is at `../agent-skills/skills` or at the path in `AGENT_SKILLS_DIR`. If neither exists, those tests print `SKIPPED` and pass.

## Not implemented yet (roadmap)

- **Model gateway**: dispatch contracts to hosted and local models, using `routing` to pick the tier, with budgets and cost tracking.
- **Review loop**: automated checks, then `Verdict`s, then the REVISE/REASSIGN escalation ladder.
- **24/7 scheduling**: long-running and scheduled runs with a kill switch.
- **Agent messaging**: a structured task board or queue between agents, plus notifications.
- **Audit log**: write `runlog` entries for every dispatched task.

## License

MIT. See [LICENSE](LICENSE).

# Leafhill Project Configuration

Place this file in your project root as `leafhill.config.md` and fill in the values below. Any setting left blank or removed will use the default from the leafhill_dev skill.

---

## Project Info

name:
description:

## Project Type

type:

Options: `personal` | `open-source` | `company`
Default: `personal`

## Version Control

vcs:

Options: `git` | `none`
Default: `git`

branching:

Options: `simple` | `trunk-based` | `gitflow`
Default: `simple`

commit_style:

Options: `conventional` | `free-form`
Default: `conventional`

## Languages

primary:

Options: `python` | `javascript` | `typescript` | `go` | `rust` | `other`
Default: (auto-detect)

## Testing

test_framework:

Specify your testing framework (e.g., `pytest`, `jest`, `vitest`, `go test`). Leave blank to auto-detect.

## Companion Tools

roam_code:

Options: `on` | `off`
Default: `on`
Codebase navigation and context gathering. Set to `off` to disable.

superpowers:

Options: `on` | `off`
Default: `on`
Workflow orchestration (brainstorming, debugging, TDD, code review). Set to `off` to disable.

persistent_memory:

Options: `on` | `off`
Default: `on`
Cross-session task tracking and project memory via the leafhill-persistent-memory MCP server. Requires the MCP server to be configured and running. Set to `off` to disable.

## Additional Rules

Add any project-specific rules below. The AI will follow these in addition to the leafhill_dev defaults.

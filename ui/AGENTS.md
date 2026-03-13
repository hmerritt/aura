# AGENTS

Short rules for working in this repo.

## Scope

- This is a Vite + React + TypeScript template with TanStack Store/Router and StyleX.
- Bun is used as the package manager, and script-runner
- All changes must be clean and testable.

## Setup

- See `package.json` for all available scripts
- Install: `bun install`
- Dev: `bun dev`
- Build: `bun build`
- Lint: `bun lint`
- Unit tests: `bun test`
- E2E tests: `bun test:e2e` (run `bun test:e2e:setup` once)
- Cosmos (a storybook alternative for testing components): `bun cosmos`

## Structure

- `src/lib` shared utilities, hooks, styles, globals.
- `src/state` TanStack Store slices, actions, persistence.
- `src/view` UI components and routes.
- `tests-e2e` Playwright tests.
- `bootstrap.ts` wraps tool commands for consistent config.

## Conventions

- TypeScript only; keep types explicit at module boundaries. Prefer strict typing; avoid `any`
- react-compiler is active. Do not use `useMemo` or `useCallback` unless absolutely necessary
- Add brief code comments for tricky or non-obvious logic.
- Prefer existing hooks/utilities in `src/lib` before adding new ones.
- New UI components go under `src/view/components`; export from `src/view/components/index.ts`.
- New routes go under `src/view/routes` and into the TanStack route tree.

## Styling and StyleX

- See StyleX SKILL `.agents/skills/stylex/SKILL.md`
- Use StyleX for styling; do not add ad-hoc CSS files.
- Custom components have an `sx` prop to pass StyleX styles
- Use the shared `SxProp` type from `src/lib/type-assertions.ts` at component boundaries
- Do not merge `className` for StyleX composition; use `stylex.props(...)` and `sx`
- Reuse existing tokens in `src/lib/styles/colors.stylex.ts`, `src/lib/styles/shadows.stylex.ts`, `src/lib/styles/variables.stylex.ts`, and `src/lib/styles/keyframes.stylex.ts` before adding new ones
- Keep dynamic values in StyleX-safe patterns:
    - Use `stylex.create` function styles for dynamic style arguments.
    - Use CSS custom properties and StyleX vars for values that must vary at runtime.
- Avoid non-static values in raw style objects.
- Quick implementation playbook
    1. Reuse existing vars and mixins first.
    2. Add local `stylex.create` map with small, semantic keys.
    3. Compose variants through conditional `stylex.props`.
    4. Expose `sx` only when the component is intended for extension.
    5. Put `sx` last.
    6. Run lint/tests for changed surfaces.

## State & Router

- Add new state as a slice in `src/state/slices`.
- Mutations go through actions; avoid direct state writes outside store logic.
- Routing uses TanStack Router; follow the generated route tree pattern.

The state is built from individual slices defined in `src/state/slices`. A slice is a way of namespacing state within the store.
In this context, slices are used only to organize big state objects (they do not limit functionality in any way).
An action in any slice can change the state of any other slice. This differs from Redux's `combineReducers`,
which can NOT be used to change the state of other reducers.

Create a new slice by creating a directory in `src/state/slices` with `[name]Store` and `[name]Actions` files.
The store file only contains an object (the initial state) for that slice. The actions contain functions that update the state.
Ideally, state updates should only be made from within actions. This ensures state updates are predictable.

Re-export actions in `src/state/actions.ts` file for the new slice (this makes importing actions easier, `import {xyz} from 'state/actions`).

Finally, slices are then combined into the main store, in `src/state/store.ts`.

## Tests

- Tests are written in `Vitest`
- Everything should be tested, and be testable
- New behavior requires tests (unit or e2e).
- Keep tests small and deterministic.

### Test tips

A few tips to write better tests:

[Russ Cox - Go Testing By Example](https://www.youtube.com/watch?v=1-o-iJlL4ak)

- Make it easy to add new tests.
- Use test coverage to find untested code.
- Coverage is no substitute for thought.
- Write exhaustive tests.
- Separate test cases from test logic (i.e use test case tables, separate from logic).
- Look for special cases.
- If you didn't add a test, you didn't fix the bug.
- Test cases can be in testdata files.
- Compare against other implementations.
- Make test failures readable.
- If the answers can change, wtite coed to update them.
- Code quality is limited by test quality.
- Scripts make good test cases.
- Improve your tests over time.

## Commits

- Follow `CONTRIBUTING.md` prefix rules and lowercase messages.

## Agent-Specific Notes

- When answering questions, respond with high-confidence answers only: verify in code; do not guess.

# aura

- [Vite](https://vitejs.dev)
- [Vitest](https://vitest.dev/) (testing for Vite)
- [Typescript](https://www.typescriptlang.org)
- [TanStack Store](https://tanstack.com/store/latest)
- [TanStack Router](https://tanstack.com/router/latest)
- [StyleX](https://stylexjs.com/)
- [React Scan](https://github.com/aidenybai/react-scan) (local development)

## Getting started

**_Quick start_**, get up an running in one command:

```bash
bun i && bun dev
```

Clone this repo and run one of the following scripts:

Available scripts (run using `bun run <script>`):

- `dev` - starts Vite dev server for local development
- `lint` - runs fast lint (`oxlint`), ESLint compatibility checks, and type-checking
- `lint:fast` - runs `oxlint` (primary fast lint pass)
- `lint:eslint` - runs ESLint (StyleX + React hooks/compiler compatibility rules)
- `typecheck` - runs TypeScript type-checking (`tsc --noEmit`)
- `test` - runs all test files
- `preview` - similar to `dev`, but uses production mode to simulate the final build
- `build` - builds the project to `dist` directory

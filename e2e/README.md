This directory contains some WebDriver tests for `pgpad`.

## Dependencies

- `tauri-driver`
  - `cargo install tauri-driver`
- The Rust and npm dependencies necessary to build `pgpad` itself

## Running

```bash
cd e2e
npm install
npm test
```

`npm test` runs `wdio`, which:

1. builds the debug Tauri binary (`npm run tauri build -- --debug --no-bundle`)
2. starts `tauri-driver`
3. runs specs under `e2e/specs/`

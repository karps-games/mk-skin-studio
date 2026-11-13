# Contributing

Thanks for helping out. Small, focused changes are the easiest to review and merge.

## Reporting bugs

Open an issue or post in our [Discord](https://discord.gg/93vfR7597T). Please include:

- your OS and the MK Skin Studio version (shown in the sidebar)
- the game format you used (`.iso`, `.wbfs`, `.rvz`, `.wia` or extracted folder) and its region
- what you did, what you expected, and what happened instead
- a screenshot if it is a visual problem

Never attach game files, disc images or exported mods made from them.

## Development setup

Install the prerequisites listed in [Building from source](README.md#building-from-source), then:

```sh
npm ci
npm run tauri dev
```

## Tests

Run these before opening a pull request:

```sh
npx tsc --noEmit
npm run test:i18n
cd src-tauri && cargo test --lib
```

The UI suite drives the real frontend in headless Chrome or Edge with a synthetic fixture (no game files needed):

```sh
npm run test:ui
```

Set `MKSS_BROWSER` to the browser executable if it is not found automatically. Screenshots are written to `design/screens/`.

Some Rust tests read a real game. They are skipped unless you point them at your own extracted copy:

```sh
MKSS_TEST_GAME_DIR=/path/to/extracted/files MKSS_TEST_SCRATCH=/path/to/temp cargo test --lib -- --test-threads=1
```

`MKSS_TEST_IMAGE=/path/to/your/game.wbfs` also enables the disc image tests (`-- --ignored`).

## Code style

- TypeScript runs in strict mode and must pass `tsc --noEmit`.
- Keep code readable through clear names and small functions instead of comments.
- Match the formatting of the file you are editing.

## Translations

The app ships in English, Portuguese, Spanish, French and Japanese (`src/i18n/*.ts`). Every locale must have exactly the same keys. `npm run test:i18n` checks this and also finds keys that are used in `src/` but missing.

To add or improve a translation, edit the locale file and run the i18n test.

## Game files

Never commit game files or anything extracted from a game: disc images, `.szs`, `.brres`, textures, models, menu portraits or screenshots that show game art. The `.gitignore` blocks the common formats, but please double check your changes.

## Pull requests

- One topic per pull request, with a short description of what changed and why.
- Include tests for bug fixes and new behavior when it is practical.
- Make sure the tests above pass.

By contributing, you agree that your work is licensed under the GPL-3.0-or-later, like the rest of the project.

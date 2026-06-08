# Scripts

This directory contains utility scripts for maintaining the Gimoji project.

## serve-web.sh

Builds the `gimoji-web` WASM crate, bundles it with `wasm-bindgen`, runs `wasm-opt -Oz` when
binaryen is installed, and serves the result at <http://localhost:8000>.

### Usage

```bash
# Run from the project root directory
./scripts/serve-web.sh           # serves on port 8000
./scripts/serve-web.sh 4200      # serves on the specified port
```

The script installs the `wasm32-unknown-unknown` Rust target and a matching `wasm-bindgen-cli`
version automatically on first run. `wasm-opt` is optional (the bundle will be larger without
it but still functional). The generated bundle lives at `crates/gimoji-web/web/dist/`, which is
gitignored.

## update-emojis

A Rust application that automatically updates the emoji database from the upstream
[gitmoji](https://github.com/carloscuesta/gitmoji) repository.

### Usage

```bash
# Run from the scripts directory (normal output)
cd scripts
cargo run

# Run with GitHub Actions output format
cargo run -- --github-actions

# Show help
cargo run -- --help
```

### What it does

1. **Fetches** the latest gitmoji database from the upstream repository
2. **Merges** it with our current `emojis.json` file using the following strategy:
   - Upstream gitmoji entries take priority over duplicates
   - Custom emojis not found upstream are preserved
   - The existing schema and structure is maintained
3. **Updates** `emojis.json` only if changes are detected
4. **Reports** a summary of changes including:
   - Total emoji count
   - Number of upstream emojis
   - Number of custom emojis preserved

### Merge Strategy

When merging databases, the script handles conflicts by:

- **Priority**: Upstream gitmoji entries always take precedence
- **Preservation**: Custom emojis with unique codes are kept
- **Deduplication**: Emojis are identified by their `:code:` field
- **Schema**: The original `$schema` field and structure are maintained

### Prerequisites

- Rust toolchain (cargo)
- Internet connection to fetch upstream database
- Write permissions to the project directory

### Automation

This Rust application is also used by the GitHub Actions workflow
(`.github/workflows/update-emojis.yml`) that runs automatically:

- **Scheduled**: First day of every month at 02:00 UTC
- **Manual**: Can be triggered via GitHub Actions interface

The workflow will create a pull request when updates are available. When run by the workflow, the
tool uses the `--github-actions` flag to output GitHub Actions-compatible format.

### Error Handling

The application will exit with an error code if:

- The upstream database cannot be fetched
- The JSON format is invalid
- File permissions prevent writing
- The current `emojis.json` file is missing or corrupted

### Example Output

```
🚀 Starting emoji database update...

📖 Reading current database...
📡 Fetching upstream database...

📊 Current database: 75 emojis
📡 Upstream database: 73 emojis
✨ Merged database: 76 emojis
🆕 New emojis from upstream: 1
   • ✈️ :airplane: - Improve offline support.
🎨 Custom emojis preserved: 2
   • 🤖 :robot: - Changes related to automation/bots.
   • 🔌 :electric_plug: - Add or update code related to connectivity.

✅ Database updated successfully!
📊 Summary:
   • Total emojis: 76
   • From upstream: 73
   • New from upstream: 1
   • Custom preserved: 2
```

When run with `--github-actions`, the tool additionally outputs GitHub Actions-compatible format:
```
::set-output name=has_changes::true
::set-output name=total_emojis::76
::set-output name=upstream_emojis::73
::set-output name=custom_emojis::2
::set-output name=new_upstream_emojis::1
```

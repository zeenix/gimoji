# gimoji

[![Build Status](https://github.com/zeenix/gimoji/actions/workflows/rust.yml/badge.svg)](https://github.com/zeenix/gimoji/actions/workflows/rust.yml)
[![gitmoji badge](https://img.shields.io/badge/gitmoji-%20😜%20😍-FFDD67.svg?style=flat-square)](https://github.com/carloscuesta/gitmoji)

![./screenshot](screenshot.png)

A CLI tool that makes it easy to add emojis to your git commit messages. It's very similar to (and
is based on) [gitmoji-cli] but written in Rust.

## Installation

```bash
cargo install -f gimoji
```

## Usage

`gimoji` is primarily intended to be used as a git commit hook. Once installed, ask `gimoji` to
install the hook in your respositry:

```bash
cd /path/to/your/project/
gimoji --init
```

Now, whenever you run `git commit`, `gimoji` will kick in and prompt you to choose an emoji.

If you launch `gimoji` directly without any arguments, it will prompt you to choose an emoji and
then copy your choice to the system clipboard.

Use `--help` to see all the available options.

## Updating the emoji cache

On the first run, `gimoji` will download the emoji list from [gitmoji] and cache it locally. If you
want to update the cache, run:

```bash
gimoji --update-cache
```

## Rationale

[gitmoji-cli] while being a great tool, can be considerably [slow]. Hence this project. `gimoji` has a
few differences:

* it will launch a full-screen terminal UI to choose an emoji, hence emojis on the console.
* it will only add an emoji to the commit if it's a completely new commit without any existing
  message (e.g it won't kick in when a message is already specified through `-m` option of
  `git commit`, or when ammending a commit).
* it does not add anything other than an emoji (like scope, summary etc.) to the commit message and
  lets you do that in your preferred editor.

The philosophy here is to enable you to quickly and easily choose an emoji and get out of your way.

## License

[MIT](LICENSE)

[gitmoji]: https://github.com/carloscuesta/gitmoji
[gitmoji-cli]: https://github.com/carloscuesta/gitmoji-cli
[slow]: https://github.com/carloscuesta/gitmoji-cli/issues/1096

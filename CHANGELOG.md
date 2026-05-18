# pgCrab Changelog

<!-- top -->

## pgCrab 0.2.0 (2026-05-18)

### Removals and Deprecations

- Remove SQL linting functionality (`pgcrab lint-sql` command), which would find and lint SQL statements embedded in Python or Rust source code. ([\#3](https://git.emunest.net/reivilibre/pgcrab/pulls/3))

### Features

- Return non-zero exit code when there are diagnostics. ([\#4](https://git.emunest.net/reivilibre/pgcrab/pulls/4))

### Internal Changes

- Fix clippy lints. ([\#5](https://git.emunest.net/reivilibre/pgcrab/pulls/5))


## pgCrab v0.1.0 (2026-05-17)

Initial versioned release, with:

- schema documentation generation;
- schema linting; and
- SQL-in-Rust and SQL-in-Python linting (proof of concept).

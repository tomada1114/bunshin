# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Local civil time alongside each clock instant, with a tunable 04:00 boundary for
  the logical day and fixed-offset clocks for deterministic tests.

### Removed

- **Breaking:** removed the counter sample, its public APIs, and the `counter`
  subcommand. Stored `counter.json` files are ignored and left untouched.

### Changed

- Core's `Clock::now` returns `Now`; counter timestamps keep their existing Unix
  millisecond format. Core rejects direct clock reads through jiff as well as `std`.

- `bunshin tui` now opens an empty-day shell with `q` and Ctrl+C to quit.

# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.6.0](https://github.com/near/borsh-rs/compare/borsh-v1.5.0...borsh-v1.6.0) - 2024-05-30

### Added
- *(schema)* for `HashMap<K, V>` -> `HashMap<K, V, S>`, for `HashSet<T>` -> `HashSet<T, S>` ([#294](https://github.com/near/borsh-rs/pull/294))

### Fixed
- fixed linting warnings for Rust 1.78 stable,  1.80 nightly ([#295](https://github.com/near/borsh-rs/pull/295))

# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-18

### Added

- Decompiler for Microsoft Flight Simulator 2024 compiled `.spb` property files.
- Property definition loader that scans a propdefs directory and resolves GUIDs.
- Automatic propdefs discovery on Windows for the Microsoft Store / Xbox and
  Steam installs, with `--propdefs` and the `SPB2XML_PROPDEFS` environment
  variable as overrides.
- Support for the 2024 value size field, used to decode float input pins that
  store a source GUID alongside their floats.
- Embedded text decoding table, so the binary has no runtime data dependencies.
- Single file and recursive directory conversion.
- UTF-8 and Windows-1252 output encodings.
- Library API exposing `convert`, `Bank`, `TextTable` and `Encoding`.
- Case insensitive value type matching, covering mixed case propdef spellings
  such as `Float` and `Bool`.

### Credits

- Based on [leppie/spb2xml](https://github.com/leppie/spb2xml). The embedded
  text decoding table is derived from its `TextDecode.Data.cs`.


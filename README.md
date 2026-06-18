# spb2xml24

[![CI](https://github.com/vectorial287/spb2xml24/actions/workflows/ci.yml/badge.svg)](https://github.com/vectorial287/spb2xml24/actions/workflows/ci.yml)
[![Release](https://github.com/vectorial287/spb2xml24/actions/workflows/release.yml/badge.svg)](https://github.com/vectorial287/spb2xml24/actions/workflows/release.yml)

A standalone command line tool that decompiles Microsoft Flight Simulator 2024
compiled property files (`.spb`) back into readable XML.

Flight Simulator stores effect graphs, scenery objects, missions and many other
authored documents as compiled `SimBase` property banks. The compiler turns the
original XML into a binary `.spb` file. `spb2xml24` reverses that step so the
underlying document can be read, diffed and studied.

It is a ground up Rust reimplementation that builds on
[leppie/spb2xml](https://github.com/leppie/spb2xml), a C# decompiler for MSFS
2020. The 2024 file format records a per property value size that earlier tools
discarded. That value is what lets `spb2xml24` decode the new "input pin"
properties correctly, where a float is stored together with a source GUID. See
[Relationship to leppie/spb2xml](#relationship-to-leppiespb2xml) for the full
list of differences.

## Features

- Single self contained binary with no runtime dependencies.
- The text decoding table is embedded, so no external data files are needed.
- Converts a single file or a whole directory tree of `.spb` files.
- UTF-8 output by default, with a Windows-1252 mode that reproduces the byte
  layout the simulator's own tools emit.
- Usable both as a CLI and as a Rust library.

## Requirements

The simulator's property definition files map each GUID to a readable name. They
are not redistributed with this tool. Point `spb2xml24` at the `Common` propdefs
folder from your Flight Simulator 2024 installation, for example:

```
C:\XboxGames\Microsoft Flight Simulator 2024\Content\Propdefs\1.0\Common
```

When that default path exists it is used automatically. Otherwise pass the
location with `--propdefs`.

## Installation

Build a release binary with Cargo:

```
cargo build --release
```

The executable is written to `target/release/spb2xml24`.

## Usage

Convert one file:

```
spb2xml24 effect.spb
```

Convert one file to a named output:

```
spb2xml24 --propdefs "D:\Propdefs\1.0\Common" effect.spb effect.xml
```

Convert a directory tree and mirror it into an output folder:

```
spb2xml24 --recursive --out out_dir VisualEffectLib
```

### Options

| Option | Description |
| --- | --- |
| `-s`, `--propdefs <dir>` | Property definition directory. Defaults to the Flight Simulator 2024 Common propdefs when present. |
| `-o`, `--out <path>` | Output file, or output directory for a directory input. |
| `-e`, `--encoding <enc>` | Output encoding: `utf-8` (default) or `windows-1252`. |
| `-r`, `--recursive` | Recurse into subdirectories of a directory input. |
| `-v`, `--verbose` | Print each converted file. |
| `-h`, `--help` | Show help. |
| `-V`, `--version` | Show version. |

## Output encoding

The default `utf-8` output is lossless and convenient for modern tooling. The
`windows-1252` mode matches the declaration and byte encoding used by the
simulator's native XML, which is useful when you want output that lines up with
the original authored files.

## Library use

The crate also exposes a small API:

```rust
use std::path::Path;
use spb2xml24::{convert, Bank, Encoding, TextTable};

let bank = Bank::load(Path::new("Propdefs/1.0/Common"))?;
let text = TextTable::embedded();
let spb = std::fs::read("effect.spb")?;
let xml = convert(&spb, &bank, &text, Encoding::Utf8)?;
std::fs::write("effect.xml", xml)?;
# Ok::<(), spb2xml24::Error>(())
```

## How it works

A short walk through the pipeline:

1. `Bank::load` scans the propdefs directory and builds a GUID keyed table of
   property, set and type definitions.
2. `TextTable::embedded` loads the position dependent text decoding table from
   the embedded asset.
3. `spb::parse` reads the header and tag table, then walks the document tree,
   using each property's declared type and stored value size to decode it.
4. The resulting tree is rendered to XML.

The binary layout and the value size rule are documented in
[docs/FORMAT.md](docs/FORMAT.md).

## Project layout

```
spb2xml24/
  Cargo.toml
  README.md
  CHANGELOG.md
  LICENSE
  assets/
    textdecode.bin        Embedded text decoding table
  docs/
    FORMAT.md             SPB binary format reference
  src/
    main.rs               Binary entry point
    cli.rs                Argument parsing and file traversal
    lib.rs                Library entry point and public API
    error.rs              Shared error type
    reader.rs             Little endian byte reader
    guid.rs               GUID formatting
    textdecode.rs         Text decoder over the embedded table
    propdefs.rs           Property definition loader
    spb.rs                SPB stream parser
    format.rs             Numeric and coordinate formatting
    xml.rs                XML document model and serialiser
  tests/
    integration.rs        End to end test on a synthetic document
  tools/
    extract_textdecode.py Regenerates assets/textdecode.bin
```

## Development

```
cargo test            # run the test suite
cargo fmt             # format
cargo clippy          # lint
```

To regenerate the embedded text table from a source file, such as
`TextDecode.Data.cs` from [leppie/spb2xml](https://github.com/leppie/spb2xml):

```
python tools/extract_textdecode.py path/to/TextDecode.Data.cs assets/textdecode.bin
```

## Relationship to leppie/spb2xml

[leppie/spb2xml](https://github.com/leppie/spb2xml) is a C# decompiler for MSFS
2020. `spb2xml24` reuses its hard won knowledge of the format (header and tag
layout, GUID handling, set framing, the coordinate and angle math, and the text
decoding table) and adds support for the MSFS 2024 changes.

What is new here:

| Area | leppie/spb2xml | spb2xml24 |
| --- | --- | --- |
| Float input pins | Not decoded; the per tag value size is read as an "unknown flag" and discarded | Decoded using the value size, producing `{guid},value` for the new node graph pins |
| Value types | `INPUTxxx`, `OUTPUTVALUE`, `BEZIERCURVE` and `FLOAT3` are not handled | Adds `FLOAT3`, `OUTPUTVALUE`, `BEZIERCURVE` and the full `INPUT*` family |
| Output encoding | Windows-1252 only | UTF-8 (default) and Windows-1252 |
| Runtime | Requires the .NET runtime | Single static binary, no runtime or dependencies |
| Use as a library | CLI only | CLI plus a Rust crate API |

What leppie/spb2xml still does that this tool does not:

- Caches parsed propdefs to a `propdefs.cache` file between runs.
- Accepts a model list (`-m`) to annotate model GUIDs with friendly names.

## Credits

- [leppie/spb2xml](https://github.com/leppie/spb2xml) for the MSFS 2020 C#
  decompiler this work is based on, and in particular for `TextDecode.Data.cs`,
  the source of the embedded text decoding table in `assets/textdecode.bin`.

## License

Released under the MIT License. See [LICENSE](LICENSE).


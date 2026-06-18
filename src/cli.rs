//! Command line front end: argument parsing and file or directory traversal.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use spb2xml24::{convert, Bank, Encoding, Result, TextTable};

const HELP: &str = "\
spb2xml24 - decompile MSFS 2024 SPB property files to XML

USAGE:
    spb2xml24 [OPTIONS] <input> [output]

ARGS:
    <input>     An .spb file or a directory of .spb files
    [output]    Output file, or output directory when <input> is a directory

OPTIONS:
    -s, --propdefs <dir>    Propdefs directory. Auto-detected from the MSFS 2024
                            install (Microsoft Store/Xbox or Steam) when omitted.
    -o, --out <path>        Output file or directory (same as the [output] arg)
    -e, --encoding <enc>    Output encoding: utf-8 (default) or windows-1252
    -r, --recursive         Recurse into subdirectories of a directory input
    -v, --verbose           Print each converted file
    -h, --help              Show this help
    -V, --version           Show version information

ENVIRONMENT:
    SPB2XML_PROPDEFS        Propdefs directory, used when --propdefs is omitted
                            and taking precedence over auto-detection.

EXAMPLES:
    spb2xml24 effect.spb
    spb2xml24 --propdefs \"D:\\Propdefs\\1.0\\Common\" effect.spb effect.xml
    spb2xml24 --recursive --out out_dir VisualEffectLib
";

/// Parsed command line options.
struct Options {
    propdefs: Option<PathBuf>,
    out: Option<PathBuf>,
    encoding: Encoding,
    recursive: bool,
    verbose: bool,
    input: Option<PathBuf>,
}

/// Program entry point. Returns a process exit code.
pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(Outcome::Run(options)) => options,
        Ok(Outcome::Message(text)) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };

    match execute(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

enum Outcome {
    Run(Options),
    Message(String),
}

fn parse_args(args: &[String]) -> std::result::Result<Outcome, String> {
    let mut propdefs = None;
    let mut out = None;
    let mut encoding = Encoding::Utf8;
    let mut recursive = false;
    let mut verbose = false;
    let mut positional: Vec<PathBuf> = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Outcome::Message(HELP.to_string())),
            "-V" | "--version" => {
                return Ok(Outcome::Message(format!(
                    "{} {}\n",
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION")
                )))
            }
            "-r" | "--recursive" => recursive = true,
            "-v" | "--verbose" => verbose = true,
            "-s" | "--propdefs" => propdefs = Some(PathBuf::from(next(&mut iter, arg)?)),
            "-o" | "--out" => out = Some(PathBuf::from(next(&mut iter, arg)?)),
            "-e" | "--encoding" => encoding = parse_encoding(&next(&mut iter, arg)?)?,
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option: {other}"));
            }
            _ => positional.push(PathBuf::from(arg)),
        }
    }

    if positional.len() > 2 {
        return Err("expected at most one input and one output path".to_string());
    }
    let input = positional.first().cloned();
    if out.is_none() {
        out = positional.get(1).cloned();
    }

    if input.is_none() {
        return Ok(Outcome::Message(HELP.to_string()));
    }

    Ok(Outcome::Run(Options {
        propdefs,
        out,
        encoding,
        recursive,
        verbose,
        input,
    }))
}

fn execute(options: Options) -> Result<()> {
    let input = options.input.expect("input is present after parsing");

    let propdefs = resolve_propdefs(options.propdefs).ok_or_else(|| {
        spb2xml24::Error::Propdefs(
            "could not locate the MSFS 2024 propdefs; pass --propdefs <dir> or set SPB2XML_PROPDEFS"
                .to_string(),
        )
    })?;

    let bank = Bank::load(&propdefs)?;
    let text = TextTable::embedded();

    if input.is_dir() {
        let input_root = input.clone();
        let output_root = match options.out {
            Some(path) => path,
            None => with_suffix(&input_root, "_xml"),
        };
        let mut count = 0usize;
        for spb in collect_spb(&input_root, options.recursive)? {
            let output = output_path(&spb, Some(&input_root), Some(&output_root));
            convert_file(&spb, &output, &bank, &text, options.encoding)?;
            if options.verbose {
                println!("{} -> {}", spb.display(), output.display());
            }
            count += 1;
        }
        println!("Converted {count} SPB file(s) to {}", output_root.display());
    } else {
        let output = output_path(&input, None, options.out.as_deref());
        convert_file(&input, &output, &bank, &text, options.encoding)?;
        println!("Wrote {}", output.display());
    }
    Ok(())
}

fn convert_file(
    input: &Path,
    output: &Path,
    bank: &Bank,
    text: &TextTable,
    encoding: Encoding,
) -> Result<()> {
    let data = std::fs::read(input)?;
    let xml = convert(&data, bank, text, encoding)?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(output, xml)?;
    Ok(())
}

/// Compute the output path for one input, mirroring the reference tool.
fn output_path(input: &Path, input_root: Option<&Path>, output_root: Option<&Path>) -> PathBuf {
    if let Some(root) = input_root {
        let relative = input.strip_prefix(root).unwrap_or(input);
        return to_xml(&output_root.unwrap_or(input).join(relative));
    }
    match output_root {
        None => to_xml(input),
        Some(out) if out.is_dir() || out.extension().is_none() => {
            let name = input.file_name().map(PathBuf::from).unwrap_or_default();
            to_xml(&out.join(name))
        }
        Some(out) => out.to_path_buf(),
    }
}

fn to_xml(path: &Path) -> PathBuf {
    path.with_extension("xml")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

fn collect_spb(root: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                if recursive {
                    stack.push(path);
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("spb"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn parse_encoding(value: &str) -> std::result::Result<Encoding, String> {
    match value.to_ascii_lowercase().as_str() {
        "utf-8" | "utf8" => Ok(Encoding::Utf8),
        "windows-1252" | "windows1252" | "cp1252" | "1252" | "ansi" => Ok(Encoding::Windows1252),
        other => Err(format!(
            "unknown encoding '{other}' (use utf-8 or windows-1252)"
        )),
    }
}

/// Resolve the propdefs directory: an explicit `--propdefs`, then the
/// `SPB2XML_PROPDEFS` environment variable, then auto-detection. The chosen
/// directory is reported when it was not given on the command line.
fn resolve_propdefs(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(dir) = explicit {
        return Some(dir);
    }
    if let Some(dir) = std::env::var_os("SPB2XML_PROPDEFS").map(PathBuf::from) {
        eprintln!("Using propdefs from SPB2XML_PROPDEFS: {}", dir.display());
        return Some(dir);
    }
    let dir = spb2xml24::locate::find_propdefs()?;
    eprintln!("Auto-detected propdefs: {}", dir.display());
    Some(dir)
}

fn next<'a>(
    iter: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> std::result::Result<String, String> {
    iter.next()
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

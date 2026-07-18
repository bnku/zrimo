use std::{env, fs, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(input) = arguments.next() else {
        eprintln!("usage: convert <input.doc|xls|ppt> <output> [doc|xls|ppt]");
        return ExitCode::FAILURE;
    };
    let Some(output) = arguments.next() else {
        eprintln!("usage: convert <input.doc|xls|ppt> <output> [doc|xls|ppt]");
        return ExitCode::FAILURE;
    };
    let explicit_format = arguments.next();
    if arguments.next().is_some() {
        eprintln!("usage: convert <input.doc|xls|ppt> <output> [doc|xls|ppt]");
        return ExitCode::FAILURE;
    }
    let format = explicit_format
        .as_deref()
        .and_then(std::ffi::OsStr::to_str)
        .or_else(|| {
            Path::new(&input)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
        })
        .map(str::to_ascii_lowercase);
    let Some(format) = format.filter(|format| matches!(format.as_str(), "doc" | "xls" | "ppt"))
    else {
        eprintln!("input extension or explicit format must be doc, xls or ppt");
        return ExitCode::FAILURE;
    };
    let result = fs::read(input)
        .map_err(|error| error.to_string())
        .and_then(|bytes| legacy_office_wasm::convert_legacy_bytes(&bytes, &format))
        .and_then(|bytes| fs::write(output, bytes).map_err(|error| error.to_string()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

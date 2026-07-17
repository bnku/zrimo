use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(input) = arguments.next() else {
        eprintln!("usage: convert <input.doc> <output.docx>");
        return ExitCode::FAILURE;
    };
    let Some(output) = arguments.next() else {
        eprintln!("usage: convert <input.doc> <output.docx>");
        return ExitCode::FAILURE;
    };
    if arguments.next().is_some() {
        eprintln!("usage: convert <input.doc> <output.docx>");
        return ExitCode::FAILURE;
    }
    let result = fs::read(input)
        .map_err(|error| error.to_string())
        .and_then(|bytes| legacy_office_wasm::convert_legacy_bytes(&bytes, "doc"))
        .and_then(|bytes| fs::write(output, bytes).map_err(|error| error.to_string()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

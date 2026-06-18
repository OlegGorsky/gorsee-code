use std::{
    ffi::OsString,
    io::{self, Write},
};

use anyhow::Result;
use gorsee_code_cli::{auth, run_with_options, CliOptions};

fn main() -> Result<()> {
    let args: Vec<OsString> = std::env::args_os().collect();
    let options = CliOptions::current()?;
    prompt_for_key_if_needed(&args, &options)?;

    let output = run_with_options(args, options)?;
    print!("{output}");
    Ok(())
}

fn prompt_for_key_if_needed(args: &[OsString], options: &CliOptions) -> Result<()> {
    if args.len() != 1 || auth::api_key(&options.root, options.env_key.as_deref())?.is_some() {
        return Ok(());
    }

    print!("NeuroGate API key: ");
    io::stdout().flush()?;

    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    let key = key.trim();
    if !key.is_empty() {
        auth::set(&options.root, key)?;
    }
    Ok(())
}

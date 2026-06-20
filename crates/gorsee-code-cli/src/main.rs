use std::ffi::OsString;

use anyhow::Result;
use gorsee_code_cli::{auth, run_interactive_tui, run_with_options, secret_prompt, CliOptions};

fn main() -> Result<()> {
    let args: Vec<OsString> = std::env::args_os().collect();
    let options = CliOptions::current()?;
    prompt_for_key_if_needed(&args, &options)?;

    if opens_interactive_tui(&args) {
        run_interactive_tui(&options)?;
        return Ok(());
    }

    let output = run_with_options(args, options)?;
    print!("{output}");
    Ok(())
}

fn prompt_for_key_if_needed(args: &[OsString], options: &CliOptions) -> Result<()> {
    if !opens_interactive_tui(args)
        || auth::api_key(&options.root, options.env_key.as_deref())?.is_some()
    {
        return Ok(());
    }

    let key = secret_prompt::read_api_key()?;
    let key = key.trim();
    if !key.is_empty() {
        auth::set(&options.root, key)?;
    }
    Ok(())
}

fn opens_interactive_tui(args: &[OsString]) -> bool {
    args.len() == 1 || (args.len() == 2 && args[1] == "tui")
}

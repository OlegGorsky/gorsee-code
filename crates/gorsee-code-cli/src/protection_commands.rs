use std::path::Path;

use anyhow::{bail, Result};

use crate::{args::ProtectArgs, config_file};

pub fn protect(root: &Path, args: ProtectArgs) -> Result<String> {
    let mut config = config_file::load_editable(root)?;
    let mut added = 0_u64;

    for path in args.paths.into_iter().map(|path| path.trim().to_string()) {
        if path.is_empty() || config.project.protected_paths.contains(&path) {
            continue;
        }
        config.project.protected_paths.push(path);
        added += 1;
    }

    if config.project.protected_paths.is_empty() {
        bail!("protect needs at least one path");
    }

    config_file::save(root, &config)?;
    Ok(format!(
        "protect: updated\npaths={}\nadded={added}\n",
        config.project.protected_paths.len()
    ))
}

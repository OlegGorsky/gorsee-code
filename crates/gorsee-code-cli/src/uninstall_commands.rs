use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::{
    args::{UninstallArgs, UserDataMode},
    paths,
};

pub fn run(root: &Path, args: UninstallArgs) -> Result<String> {
    remove_file_if_exists(paths::config_path(root))?;
    match args.user_data {
        UserDataMode::Keep => Ok("uninstall: complete\nuser_data=kept\n".into()),
        UserDataMode::Remove => {
            remove_dir_if_exists(paths::local_dir(root))?;
            Ok("uninstall: complete\nuser_data=removed\n".into())
        }
    }
}

fn remove_dir_if_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

fn remove_file_if_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

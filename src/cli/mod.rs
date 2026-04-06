mod init;
mod install;
mod package;

pub use init::dev_init;
pub use install::{dev_install, dev_install_url};
pub use package::dev_package;

use anyhow::Result;
use std::path::Path;

/// Copy directory contents, skipping manifest and dotfiles.
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str == "accessforge.yml" || name_str.starts_with('.') {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    File,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    path: PathBuf,
    label: String,
    kind: AttachmentKind,
}

impl Attachment {
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        let path = path.as_ref();
        if !path.is_file() {
            return None;
        }
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| path.to_str().unwrap_or("file"))
            .to_string();
        Some(Self {
            path: path.to_path_buf(),
            label,
            kind: kind_for_path(path),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn kind(&self) -> AttachmentKind {
        self.kind
    }
}

pub(crate) fn attachment_from_paste(text: &str) -> Option<Attachment> {
    let path = text.trim();
    if path.contains('\n') || path.is_empty() {
        return None;
    }
    Attachment::from_path(path)
}

fn kind_for_path(path: &Path) -> AttachmentKind {
    let is_image = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
            )
        })
        .unwrap_or(false);
    if is_image {
        AttachmentKind::Image
    } else {
        AttachmentKind::File
    }
}

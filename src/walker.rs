use ignore::WalkBuilder;
use std::path::Path;

pub struct FileWalker {
    pub respect_gitignore: bool,
    pub include_hidden: bool,
}

impl FileWalker {
    pub fn new(respect_gitignore: bool, include_hidden: bool) -> Self {
        Self {
            respect_gitignore,
            include_hidden,
        }
    }

    pub fn walk<P: AsRef<Path>>(&self, root: P) -> impl Iterator<Item = ignore::DirEntry> {
        WalkBuilder::new(root)
            .hidden(!self.include_hidden)
            .git_ignore(self.respect_gitignore)
            .git_global(self.respect_gitignore)
            .git_exclude(self.respect_gitignore)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
    }
}

//! Shared constants describing where per-instance config files live and
//! which file extensions count as "config" for the purposes of file
//! watching and mrpack installation.

pub(crate) const CONFIG_DIRECTORY: &str = "config";
pub(crate) const CONFIG_FILE_EXTENSIONS: [&str; 14] = [
    "json",
    "json5",
    "jsonc",
    "yml",
    "yaml",
    "css",
    "toml",
    "txt",
    "ini",
    "cfg",
    "conf",
    "properties",
    "xml",
    "nbt",
];

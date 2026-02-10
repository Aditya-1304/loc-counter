use std::collections::HashMap;

#[derive(Debug)]
pub struct LanguageConfig {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub line_comment: Option<&'static str>,
    pub block_comment: Option<(&'static str, &'static str)>,
}

static LANGUAGES: &[LanguageConfig] = &[
    LanguageConfig {
        name: "Rust",
        extensions: &["rs"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "Python",
        extensions: &["py", "pyw"],
        line_comment: Some("#"),
        block_comment: None,
    },
    LanguageConfig {
        name: "JavaScript",
        extensions: &["js", "mjs", "cjs"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "TypeScript",
        extensions: &["ts", "tsx"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "C",
        extensions: &["c", "h"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "C++",
        extensions: &["cpp", "hpp", "cc", "cxx", "hxx"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "Java",
        extensions: &["java"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "Go",
        extensions: &["go"],
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "HTML",
        extensions: &["html", "htm"],
        line_comment: None,
        block_comment: Some(("<!--", "-->")),
    },
    LanguageConfig {
        name: "CSS",
        extensions: &["css"],
        line_comment: None,
        block_comment: Some(("/*", "*/")),
    },
    LanguageConfig {
        name: "Shell",
        extensions: &["sh", "bash", "zsh"],
        line_comment: Some("#"),
        block_comment: None,
    },
    LanguageConfig {
        name: "TOML",
        extensions: &["toml"],
        line_comment: Some("#"),
        block_comment: None,
    },
    LanguageConfig {
        name: "YAML",
        extensions: &["yaml", "yml"],
        line_comment: Some("#"),
        block_comment: None,
    },
    LanguageConfig {
        name: "JSON",
        extensions: &["json"],
        line_comment: None,
        block_comment: None,
    },
    LanguageConfig {
        name: "Markdown",
        extensions: &["md", "markdown"],
        line_comment: None,
        block_comment: None,
    },
    LanguageConfig {
        name: "Plain Text",
        extensions: &["txt", "text"],
        line_comment: None,
        block_comment: None,
    },
    LanguageConfig {
        name: "Makefile",
        extensions: &["mk", "makefile"],
        line_comment: Some("#"),
        block_comment: None,
    },
];

pub fn get_language_configs() -> HashMap<&'static str, &'static LanguageConfig> {
    let mut map = HashMap::new();
    for lang in LANGUAGES {
        for &ext in lang.extensions {
            map.insert(ext, lang);
        }
    }
    map
}

pub fn detect_language(
    extension: &str,
    configs: &HashMap<&'static str, &'static LanguageConfig>,
) -> Option<&'static LanguageConfig> {
    configs.get(extension).copied()
}

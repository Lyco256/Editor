//! Incremental Tree-sitter syntax services.
//!
//! This crate owns the syntax parsing boundary for Editor. It resolves a document's language,
//! manages per-language parsers, applies incremental edits, and extracts theme-independent
//! structural data for highlighting, folding, symbols, selections, pairs, and parse errors.

use editor_core::DocumentDescriptor;
use editor_types::{CharacterOffset, DocumentId, StyleRole, TextRange};
use std::{collections::HashMap, ops::ControlFlow, path::Path};
use tree_sitter::{InputEdit, Node, ParseOptions, Parser, Point, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseTicket {
    pub document: DocumentId,
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxLanguage {
    Rust,
    C,
    Cpp,
    CSharp,
    Java,
    Go,
    Python,
    JavaScript,
    TypeScript,
    Html,
    Css,
    Json,
    Toml,
    Yaml,
    Markdown,
    Bash,
}

impl SyntaxLanguage {
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Rust,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Java,
            Self::Go,
            Self::Python,
            Self::JavaScript,
            Self::TypeScript,
            Self::Html,
            Self::Css,
            Self::Json,
            Self::Toml,
            Self::Yaml,
            Self::Markdown,
            Self::Bash,
        ]
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "c-sharp",
            Self::Java => "java",
            Self::Go => "go",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Html => "html",
            Self::Css => "css",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Markdown => "markdown",
            Self::Bash => "bash",
        }
    }

    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        if let Some(name) = file_name.as_deref() {
            if matches!(name, "dockerfile" | "bashrc" | "profile") {
                return Some(Self::Bash);
            }
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())?
            .to_ascii_lowercase();
        match extension.as_str() {
            "rs" => Some(Self::Rust),
            "c" | "h" => Some(Self::C),
            "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "java" => Some(Self::Java),
            "go" => Some(Self::Go),
            "py" => Some(Self::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "html" | "htm" | "xhtml" => Some(Self::Html),
            "css" => Some(Self::Css),
            "json" | "jsonc" => Some(Self::Json),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            "md" | "markdown" | "mdown" => Some(Self::Markdown),
            "sh" | "bash" | "zsh" | "ksh" => Some(Self::Bash),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxStatus {
    Parsed,
    LargeFileSuppressed,
    Cancelled,
    StaleVersion,
    MissingLanguage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxSpan {
    pub range: TextRange,
    pub role: StyleRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxSymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Namespace,
    TypeAlias,
    Variable,
    Constant,
    Field,
    Property,
    Heading,
    Element,
    Rule,
    Package,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSymbol {
    pub kind: SyntaxSymbolKind,
    pub name: String,
    pub range: TextRange,
    pub selection_range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairKind {
    Paren,
    Bracket,
    Brace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxPair {
    pub kind: PairKind,
    pub open: TextRange,
    pub close: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyntaxSnapshot {
    pub ticket: Option<ParseTicket>,
    pub language: Option<SyntaxLanguage>,
    pub large_file_mode: bool,
    pub suppressed: bool,
    pub has_error: bool,
    pub highlights: Vec<SyntaxSpan>,
    pub folds: Vec<TextRange>,
    pub symbols: Vec<SyntaxSymbol>,
    pub error_regions: Vec<TextRange>,
    pub pairs: Vec<SyntaxPair>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxUpdate {
    pub status: SyntaxStatus,
    pub snapshot: SyntaxSnapshot,
    pub changed_ranges: Vec<ByteRange>,
}

impl SyntaxSnapshot {
    #[must_use]
    pub fn with_suppressed(mut self, suppressed: bool) -> Self {
        self.suppressed = suppressed;
        self
    }
}

pub trait CancellationProbe {
    fn is_cancelled(&self) -> bool;
}

impl<F> CancellationProbe for F
where
    F: Fn() -> bool,
{
    fn is_cancelled(&self) -> bool {
        self()
    }
}

pub struct OpenDocument<'a> {
    pub descriptor: DocumentDescriptor,
    pub path: Option<&'a Path>,
    pub language_override: Option<SyntaxLanguage>,
    pub text: &'a str,
}

pub struct UpdateDocument<'a> {
    pub descriptor: DocumentDescriptor,
    pub path: Option<&'a Path>,
    pub language_override: Option<SyntaxLanguage>,
    pub text: &'a str,
}

struct DocumentState {
    descriptor: DocumentDescriptor,
    language: Option<SyntaxLanguage>,
    text: String,
    tree: Option<Tree>,
    snapshot: SyntaxSnapshot,
}

struct LanguageSpec {
    language: SyntaxLanguage,
    parser: fn() -> tree_sitter::Language,
    symbol_kinds: &'static [&'static str],
    fold_kinds: &'static [&'static str],
}

fn language_spec(language: SyntaxLanguage) -> &'static LanguageSpec {
    LANGUAGE_SPECS
        .iter()
        .find(|spec| spec.language == language)
        .expect("missing language specification")
}

fn rust_language() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn c_language() -> tree_sitter::Language {
    tree_sitter_c::LANGUAGE.into()
}

fn cpp_language() -> tree_sitter::Language {
    tree_sitter_cpp::LANGUAGE.into()
}

fn csharp_language() -> tree_sitter::Language {
    tree_sitter_c_sharp::LANGUAGE.into()
}

fn java_language() -> tree_sitter::Language {
    tree_sitter_java::LANGUAGE.into()
}

fn go_language() -> tree_sitter::Language {
    tree_sitter_go::LANGUAGE.into()
}

fn python_language() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

fn javascript_language() -> tree_sitter::Language {
    tree_sitter_javascript::LANGUAGE.into()
}

fn typescript_language() -> tree_sitter::Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn html_language() -> tree_sitter::Language {
    tree_sitter_html::LANGUAGE.into()
}

fn css_language() -> tree_sitter::Language {
    tree_sitter_css::LANGUAGE.into()
}

fn json_language() -> tree_sitter::Language {
    tree_sitter_json::LANGUAGE.into()
}

fn toml_language() -> tree_sitter::Language {
    tree_sitter_toml_ng::LANGUAGE.into()
}

fn yaml_language() -> tree_sitter::Language {
    tree_sitter_yaml::LANGUAGE.into()
}

fn markdown_language() -> tree_sitter::Language {
    tree_sitter_markdown_updated::language()
}

fn bash_language() -> tree_sitter::Language {
    tree_sitter_bash::LANGUAGE.into()
}

const LANGUAGE_SPECS: &[LanguageSpec] = &[
    LanguageSpec {
        language: SyntaxLanguage::Rust,
        parser: rust_language,
        symbol_kinds: &[
            "function_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "impl_item",
            "mod_item",
            "type_item",
            "const_item",
            "static_item",
        ],
        fold_kinds: &[
            "block",
            "match_block",
            "parameters",
            "type_parameters",
            "struct_item",
            "enum_item",
            "impl_item",
            "trait_item",
            "use_list",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::C,
        parser: c_language,
        symbol_kinds: &["function_definition", "struct_specifier", "enum_specifier"],
        fold_kinds: &[
            "compound_statement",
            "parameter_list",
            "initializer_list",
            "field_declaration_list",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Cpp,
        parser: cpp_language,
        symbol_kinds: &[
            "function_definition",
            "class_specifier",
            "struct_specifier",
            "enum_specifier",
            "namespace_definition",
        ],
        fold_kinds: &[
            "compound_statement",
            "parameter_list",
            "field_declaration_list",
            "namespace_definition",
            "template_declaration",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::CSharp,
        parser: csharp_language,
        symbol_kinds: &[
            "class_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
            "method_declaration",
            "namespace_declaration",
        ],
        fold_kinds: &[
            "block",
            "class_declaration",
            "struct_declaration",
            "namespace_declaration",
            "switch_expression",
            "switch_section",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Java,
        parser: java_language,
        symbol_kinds: &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "method_declaration",
            "package_declaration",
        ],
        fold_kinds: &[
            "block",
            "class_body",
            "interface_body",
            "enum_body",
            "formal_parameters",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Go,
        parser: go_language,
        symbol_kinds: &[
            "function_declaration",
            "method_declaration",
            "type_declaration",
            "package_clause",
        ],
        fold_kinds: &[
            "block",
            "parameter_list",
            "type_parameter_list",
            "type_spec",
            "interface_type",
            "struct_type",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Python,
        parser: python_language,
        symbol_kinds: &[
            "function_definition",
            "class_definition",
            "decorated_definition",
        ],
        fold_kinds: &[
            "block",
            "parameters",
            "argument_list",
            "list",
            "dictionary",
            "tuple",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::JavaScript,
        parser: javascript_language,
        symbol_kinds: &[
            "function_declaration",
            "class_declaration",
            "method_definition",
            "lexical_declaration",
        ],
        fold_kinds: &[
            "statement_block",
            "object",
            "array",
            "formal_parameters",
            "class_body",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::TypeScript,
        parser: typescript_language,
        symbol_kinds: &[
            "function_declaration",
            "class_declaration",
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
            "method_definition",
        ],
        fold_kinds: &[
            "statement_block",
            "object",
            "array",
            "formal_parameters",
            "class_body",
            "interface_body",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Html,
        parser: html_language,
        symbol_kinds: &["element", "script_element", "style_element"],
        fold_kinds: &[
            "element",
            "script_element",
            "style_element",
            "start_tag",
            "end_tag",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Css,
        parser: css_language,
        symbol_kinds: &["rule_set", "at_rule", "keyframes_block", "declaration"],
        fold_kinds: &["block", "rule_set", "at_rule", "selector_list"],
    },
    LanguageSpec {
        language: SyntaxLanguage::Json,
        parser: json_language,
        symbol_kinds: &["pair", "object", "array"],
        fold_kinds: &["object", "array", "pair"],
    },
    LanguageSpec {
        language: SyntaxLanguage::Toml,
        parser: toml_language,
        symbol_kinds: &["table", "table_array_element", "pair"],
        fold_kinds: &["table", "table_array_element", "inline_table", "array"],
    },
    LanguageSpec {
        language: SyntaxLanguage::Yaml,
        parser: yaml_language,
        symbol_kinds: &[
            "block_mapping_pair",
            "block_sequence_item",
            "flow_mapping",
            "flow_sequence",
        ],
        fold_kinds: &[
            "block_mapping",
            "block_sequence",
            "flow_mapping",
            "flow_sequence",
            "document",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Markdown,
        parser: markdown_language,
        symbol_kinds: &[
            "atx_heading",
            "setext_heading",
            "fenced_code_block",
            "list_item",
            "block_quote",
        ],
        fold_kinds: &[
            "section",
            "atx_heading",
            "setext_heading",
            "fenced_code_block",
            "list_item",
            "block_quote",
        ],
    },
    LanguageSpec {
        language: SyntaxLanguage::Bash,
        parser: bash_language,
        symbol_kinds: &[
            "function_definition",
            "case_item",
            "command_name",
            "if_statement",
        ],
        fold_kinds: &[
            "compound_statement",
            "function_definition",
            "case_item",
            "if_statement",
            "for_statement",
            "while_statement",
        ],
    },
];

const COMMON_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "begin",
    "break",
    "case",
    "class",
    "const",
    "continue",
    "def",
    "default",
    "do",
    "done",
    "elif",
    "else",
    "enum",
    "esac",
    "export",
    "extends",
    "false",
    "final",
    "fn",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "include",
    "interface",
    "is",
    "let",
    "match",
    "mod",
    "module",
    "mut",
    "namespace",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "readonly",
    "record",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "switch",
    "then",
    "this",
    "trait",
    "true",
    "type",
    "using",
    "var",
    "while",
    "where",
];

#[derive(Default)]
pub struct SyntaxEngine {
    documents: HashMap<DocumentId, DocumentState>,
}

impl SyntaxEngine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn resolve_language(
        &self,
        path: Option<&Path>,
        override_language: Option<SyntaxLanguage>,
    ) -> Option<SyntaxLanguage> {
        override_language.or_else(|| path.and_then(SyntaxLanguage::from_path))
    }

    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn open_document(
        &mut self,
        input: OpenDocument<'_>,
        cancellation: Option<&dyn CancellationProbe>,
    ) -> SyntaxUpdate {
        self.replace_document(
            input.descriptor,
            input.path,
            input.language_override,
            input.text,
            cancellation,
        )
    }

    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn update_document(
        &mut self,
        input: UpdateDocument<'_>,
        cancellation: Option<&dyn CancellationProbe>,
    ) -> SyntaxUpdate {
        self.replace_document(
            input.descriptor,
            input.path,
            input.language_override,
            input.text,
            cancellation,
        )
    }

    #[must_use]
    pub fn snapshot(&self, document: DocumentId) -> Option<&SyntaxSnapshot> {
        self.documents.get(&document).map(|state| &state.snapshot)
    }

    #[must_use]
    pub fn selection_ranges_at(
        &self,
        document: DocumentId,
        cursor: CharacterOffset,
    ) -> Vec<TextRange> {
        let Some(state) = self.documents.get(&document) else {
            return Vec::new();
        };
        let Some(tree) = state.tree.as_ref() else {
            return Vec::new();
        };

        let byte_offset = char_to_byte_offset(&state.text, cursor.0);
        let Some(mut node) = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)
        else {
            return Vec::new();
        };

        let mut ranges = Vec::new();
        let mut previous: Option<TextRange> = None;
        loop {
            let range = text_range_for_node(&state.text, node);
            if previous != Some(range) {
                ranges.push(range);
                previous = Some(range);
            }
            let Some(parent) = node.parent() else {
                break;
            };
            node = parent;
        }
        ranges
    }

    #[must_use]
    pub fn matching_pair_at(
        &self,
        document: DocumentId,
        cursor: CharacterOffset,
    ) -> Option<SyntaxPair> {
        let state = self.documents.get(&document)?;
        state
            .snapshot
            .pairs
            .iter()
            .copied()
            .find(|pair| range_contains(pair.open, cursor) || range_contains(pair.close, cursor))
    }

    #[must_use]
    pub fn language_for_document(&self, document: DocumentId) -> Option<SyntaxLanguage> {
        self.documents
            .get(&document)
            .and_then(|state| state.language)
    }

    #[allow(clippy::too_many_lines)]
    fn replace_document(
        &mut self,
        descriptor: DocumentDescriptor,
        path: Option<&Path>,
        override_language: Option<SyntaxLanguage>,
        text: &str,
        cancellation: Option<&dyn CancellationProbe>,
    ) -> SyntaxUpdate {
        let language = self.resolve_language(path, override_language);
        let base_snapshot = SyntaxSnapshot {
            ticket: Some(ParseTicket {
                document: descriptor.id,
                version: descriptor.version,
            }),
            language,
            large_file_mode: descriptor.large_file_mode,
            ..SyntaxSnapshot::default()
        };

        if descriptor.large_file_mode {
            self.documents.insert(
                descriptor.id,
                DocumentState {
                    descriptor,
                    language,
                    text: text.to_owned(),
                    tree: None,
                    snapshot: base_snapshot.clone().with_suppressed(true),
                },
            );
            return SyntaxUpdate {
                status: SyntaxStatus::LargeFileSuppressed,
                snapshot: base_snapshot.with_suppressed(true),
                changed_ranges: Vec::new(),
            };
        }

        let Some(language) = language else {
            self.documents.insert(
                descriptor.id,
                DocumentState {
                    descriptor,
                    language: None,
                    text: text.to_owned(),
                    tree: None,
                    snapshot: base_snapshot.clone(),
                },
            );
            return SyntaxUpdate {
                status: SyntaxStatus::MissingLanguage,
                snapshot: base_snapshot,
                changed_ranges: Vec::new(),
            };
        };

        let parser = Self::parser_for(language);
        let (changed_ranges, tree) = match self.documents.get_mut(&descriptor.id) {
            Some(existing) => {
                if descriptor.version <= existing.descriptor.version {
                    return SyntaxUpdate {
                        status: SyntaxStatus::StaleVersion,
                        snapshot: existing.snapshot.clone(),
                        changed_ranges: Vec::new(),
                    };
                }

                let old_text = existing.text.clone();
                match parse_document(
                    parser,
                    text,
                    existing.tree.as_mut(),
                    &old_text,
                    cancellation,
                ) {
                    ParseDecision::Parsed {
                        changed_ranges,
                        tree,
                    } => (changed_ranges, tree),
                    ParseDecision::Cancelled => {
                        return SyntaxUpdate {
                            status: SyntaxStatus::Cancelled,
                            snapshot: existing.snapshot.clone(),
                            changed_ranges: Vec::new(),
                        };
                    }
                }
            }
            None => match parse_document(parser, text, None, "", cancellation) {
                ParseDecision::Parsed {
                    changed_ranges,
                    tree,
                } => (changed_ranges, tree),
                ParseDecision::Cancelled => {
                    return SyntaxUpdate {
                        status: SyntaxStatus::Cancelled,
                        snapshot: base_snapshot.clone(),
                        changed_ranges: Vec::new(),
                    };
                }
            },
        };

        let analysis = analyze_document(language, text, Some(&tree));
        let snapshot = SyntaxSnapshot {
            ticket: Some(ParseTicket {
                document: descriptor.id,
                version: descriptor.version,
            }),
            language: Some(language),
            large_file_mode: false,
            suppressed: false,
            has_error: analysis.has_error,
            highlights: analysis.highlights,
            folds: analysis.folds,
            symbols: analysis.symbols,
            error_regions: analysis.error_regions,
            pairs: analysis.pairs,
        };

        self.documents.insert(
            descriptor.id,
            DocumentState {
                descriptor,
                language: Some(language),
                text: text.to_owned(),
                tree: Some(tree),
                snapshot: snapshot.clone(),
            },
        );

        SyntaxUpdate {
            status: SyntaxStatus::Parsed,
            snapshot,
            changed_ranges,
        }
    }

    fn parser_for(language: SyntaxLanguage) -> Parser {
        let mut parser = Parser::new();
        parser
            .set_language(&(language_spec(language).parser)())
            .expect("Tree-sitter parser language must be loadable");
        parser
    }
}

enum ParseDecision {
    Parsed {
        changed_ranges: Vec<ByteRange>,
        tree: Tree,
    },
    Cancelled,
}

fn parse_document(
    mut parser: Parser,
    new_text: &str,
    mut old_tree: Option<&mut Tree>,
    old_text: &str,
    cancellation: Option<&dyn CancellationProbe>,
) -> ParseDecision {
    if cancellation.is_some_and(CancellationProbe::is_cancelled) {
        return ParseDecision::Cancelled;
    }

    if let Some(tree) = old_tree.as_deref_mut() {
        if !old_text.is_empty() || !new_text.is_empty() {
            if let Some(edit) = diff_to_input_edit(old_text, new_text) {
                tree.edit(&edit);
            }
        }
    }

    let bytes = new_text.as_bytes();
    let mut reader = move |byte: usize, _position: Point| {
        if byte >= bytes.len() {
            &[][..]
        } else {
            &bytes[byte..]
        }
    };

    let tree = if let Some(cancellation) = cancellation {
        let mut progress = |state: &tree_sitter::ParseState| {
            let _ = state.current_byte_offset();
            if cancellation.is_cancelled() {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let options = ParseOptions::default().progress_callback(&mut progress);
        parser.parse_with_options(&mut reader, old_tree.as_deref(), Some(options))
    } else {
        parser.parse_with_options(&mut reader, old_tree.as_deref(), None)
    };
    let Some(tree) = tree else {
        return ParseDecision::Cancelled;
    };

    let changed_ranges = old_tree
        .as_deref()
        .map(|previous| {
            previous
                .changed_ranges(&tree)
                .map(|range| ByteRange {
                    start: range.start_byte,
                    end: range.end_byte,
                })
                .collect()
        })
        .unwrap_or_default();

    ParseDecision::Parsed {
        changed_ranges,
        tree,
    }
}

fn analyze_document(language: SyntaxLanguage, text: &str, tree: Option<&Tree>) -> DocumentAnalysis {
    let Some(tree) = tree else {
        return DocumentAnalysis::default();
    };

    let root = tree.root_node();
    let mut analysis = DocumentAnalysis {
        has_error: root.has_error(),
        ..DocumentAnalysis::default()
    };
    collect_node_analysis(language, text, root, &mut analysis);
    analysis.pairs = collect_pairs(text);
    analysis
}

#[derive(Default)]
struct DocumentAnalysis {
    has_error: bool,
    highlights: Vec<SyntaxSpan>,
    folds: Vec<TextRange>,
    symbols: Vec<SyntaxSymbol>,
    error_regions: Vec<TextRange>,
    pairs: Vec<SyntaxPair>,
}

fn collect_node_analysis(
    language: SyntaxLanguage,
    text: &str,
    node: Node<'_>,
    analysis: &mut DocumentAnalysis,
) {
    if node.is_error() || node.is_missing() {
        analysis.error_regions.push(text_range_for_node(text, node));
    }

    if let Some(symbol) = symbol_for_node(language, text, node) {
        if matches!(
            symbol.kind,
            SyntaxSymbolKind::Class
                | SyntaxSymbolKind::Struct
                | SyntaxSymbolKind::Enum
                | SyntaxSymbolKind::Interface
                | SyntaxSymbolKind::Trait
                | SyntaxSymbolKind::Module
                | SyntaxSymbolKind::Namespace
                | SyntaxSymbolKind::TypeAlias
                | SyntaxSymbolKind::Heading
                | SyntaxSymbolKind::Element
                | SyntaxSymbolKind::Rule
                | SyntaxSymbolKind::Package
        ) {
            analysis.highlights.push(SyntaxSpan {
                range: symbol.selection_range,
                role: StyleRole::SemanticType,
            });
        }
        analysis.symbols.push(symbol);
    }

    if fold_for_node(language, text, node) {
        analysis.folds.push(text_range_for_node(text, node));
    }

    if let Some(span) = highlight_for_node(text, node) {
        analysis.highlights.push(span);
    }

    if node.child_count() == 0 {
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_analysis(language, text, child, analysis);
    }
}

fn highlight_for_node(text: &str, node: Node<'_>) -> Option<SyntaxSpan> {
    let kind = node.kind();
    let range = text_range_for_node(text, node);
    if kind.contains("comment") {
        return Some(SyntaxSpan {
            range,
            role: StyleRole::SyntaxComment,
        });
    }
    if kind.contains("string")
        || kind.contains("template")
        || kind.contains("raw")
        || kind.contains("char")
    {
        return Some(SyntaxSpan {
            range,
            role: StyleRole::SyntaxString,
        });
    }

    if node.child_count() == 0 {
        let token = node
            .utf8_text(text.as_bytes())
            .ok()?
            .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_');
        if COMMON_KEYWORDS.contains(&token) {
            return Some(SyntaxSpan {
                range,
                role: StyleRole::SyntaxKeyword,
            });
        }
        if matches!(token, "true" | "false" | "null" | "nil") {
            return Some(SyntaxSpan {
                range,
                role: StyleRole::SemanticType,
            });
        }
    }

    None
}

fn fold_for_node(language: SyntaxLanguage, text: &str, node: Node<'_>) -> bool {
    let range = text_range_for_node(text, node);
    if range.start == range.end || node.start_position().row == node.end_position().row {
        return false;
    }
    let kind = node.kind();
    if kind.contains("comment") {
        return false;
    }

    if language_spec(language)
        .fold_kinds
        .iter()
        .any(|candidate| kind == *candidate || kind.contains(candidate))
    {
        return true;
    }

    kind.contains("block")
        || kind.contains("body")
        || kind.contains("section")
        || kind.contains("object")
        || kind.contains("array")
        || kind.contains("table")
        || kind.contains("mapping")
        || kind.contains("sequence")
        || kind.contains("element")
        || kind.contains("list")
}

fn symbol_for_node(language: SyntaxLanguage, text: &str, node: Node<'_>) -> Option<SyntaxSymbol> {
    let kind = node.kind();
    let spec = language_spec(language);
    if !spec
        .symbol_kinds
        .iter()
        .any(|candidate| kind == *candidate || kind.contains(candidate))
    {
        return None;
    }

    let symbol_kind = infer_symbol_kind(kind);
    let name_node = node
        .child_by_field_name("name")
        .or_else(|| find_symbol_name_child(text, node));
    let name_range = name_node.map_or_else(
        || text_range_for_node(text, node),
        |child| text_range_for_node(text, child),
    );
    let name = name_node
        .map(|child| {
            child
                .utf8_text(text.as_bytes())
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| kind.to_owned());

    Some(SyntaxSymbol {
        kind: symbol_kind,
        name,
        range: text_range_for_node(text, node),
        selection_range: name_range,
    })
}

fn infer_symbol_kind(kind: &str) -> SyntaxSymbolKind {
    if kind.contains("function") {
        return SyntaxSymbolKind::Function;
    }
    if kind.contains("method") {
        return SyntaxSymbolKind::Method;
    }
    if kind.contains("class") {
        return SyntaxSymbolKind::Class;
    }
    if kind.contains("struct") {
        return SyntaxSymbolKind::Struct;
    }
    if kind.contains("enum") {
        return SyntaxSymbolKind::Enum;
    }
    if kind.contains("interface") {
        return SyntaxSymbolKind::Interface;
    }
    if kind.contains("trait") {
        return SyntaxSymbolKind::Trait;
    }
    if kind.contains("namespace") {
        return SyntaxSymbolKind::Namespace;
    }
    if kind.contains("module") || kind.contains("package") {
        return SyntaxSymbolKind::Module;
    }
    if kind.contains("type_alias")
        || kind.contains("type_item")
        || kind.contains("type_declaration")
    {
        return SyntaxSymbolKind::TypeAlias;
    }
    if kind.contains("property") {
        return SyntaxSymbolKind::Property;
    }
    if kind.contains("field") {
        return SyntaxSymbolKind::Field;
    }
    if kind.contains("heading") {
        return SyntaxSymbolKind::Heading;
    }
    if kind.contains("element") || kind.contains("tag") {
        return SyntaxSymbolKind::Element;
    }
    if kind.contains("rule") {
        return SyntaxSymbolKind::Rule;
    }
    if kind.contains("package") {
        return SyntaxSymbolKind::Package;
    }
    if kind.contains("const") || kind.contains("static") || kind.contains("variable") {
        return SyntaxSymbolKind::Constant;
    }
    SyntaxSymbolKind::Unknown
}

fn find_symbol_name_child<'a>(text: &'a str, node: Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        let kind = child.kind();
        if matches!(
            kind,
            "identifier"
                | "type_identifier"
                | "property_identifier"
                | "field_identifier"
                | "tag_name"
                | "package_identifier"
                | "name"
        ) {
            return Some(child);
        }
        if child.child_count() == 0 {
            let text = child.utf8_text(text.as_bytes()).ok()?.trim();
            if !text.is_empty() {
                return Some(child);
            }
        }
    }
    None
}

fn text_range_for_node(text: &str, node: Node<'_>) -> TextRange {
    let start = byte_to_char_offset(text, node.start_byte());
    let end = byte_to_char_offset(text, node.end_byte());
    TextRange {
        start: CharacterOffset(start),
        end: CharacterOffset(end),
    }
}

fn byte_to_char_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())].chars().count()
}

fn char_to_byte_offset(text: &str, char_offset: usize) -> usize {
    if char_offset == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_offset)
        .map_or(text.len(), |(byte, _)| byte)
}

fn byte_to_point(text: &str, byte_offset: usize) -> Point {
    let byte_offset = byte_offset.min(text.len());
    let prefix = &text[..byte_offset];
    let row = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, tail)| tail.len());
    Point { row, column }
}

fn diff_to_input_edit(old_text: &str, new_text: &str) -> Option<InputEdit> {
    if old_text == new_text {
        return None;
    }

    let old_chars: Vec<char> = old_text.chars().collect();
    let new_chars: Vec<char> = new_text.chars().collect();
    let prefix_chars = old_chars
        .iter()
        .zip(new_chars.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let suffix_chars = old_chars[prefix_chars..]
        .iter()
        .rev()
        .zip(new_chars[prefix_chars..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();

    let old_end_char = old_chars.len().saturating_sub(suffix_chars);
    let new_end_char = new_chars.len().saturating_sub(suffix_chars);

    let start_byte = char_to_byte_offset(old_text, prefix_chars);
    let old_end_byte = char_to_byte_offset(old_text, old_end_char);
    let new_end_byte = char_to_byte_offset(new_text, new_end_char);
    Some(InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: byte_to_point(old_text, start_byte),
        old_end_position: byte_to_point(old_text, old_end_byte),
        new_end_position: byte_to_point(new_text, new_end_byte),
    })
}

fn collect_pairs(text: &str) -> Vec<SyntaxPair> {
    #[derive(Clone, Copy)]
    struct OpenPair {
        kind: PairKind,
        offset: usize,
    }

    let mut pairs = Vec::new();
    let mut stack: Vec<OpenPair> = Vec::new();

    for (offset, ch) in text.chars().enumerate() {
        let kind = match ch {
            '(' | ')' => Some(PairKind::Paren),
            '[' | ']' => Some(PairKind::Bracket),
            '{' | '}' => Some(PairKind::Brace),
            _ => None,
        };
        let Some(kind) = kind else {
            continue;
        };

        match ch {
            '(' | '[' | '{' => stack.push(OpenPair { kind, offset }),
            ')' | ']' | '}' => {
                if let Some(index) = stack.iter().rposition(|candidate| candidate.kind == kind) {
                    let open = stack.remove(index);
                    pairs.push(SyntaxPair {
                        kind,
                        open: char_range(open.offset, 1),
                        close: char_range(offset, 1),
                    });
                }
            }
            _ => {}
        }
    }

    pairs
}

fn char_range(start: usize, len: usize) -> TextRange {
    TextRange {
        start: CharacterOffset(start),
        end: CharacterOffset(start + len),
    }
}

fn range_contains(range: TextRange, offset: CharacterOffset) -> bool {
    range.start <= offset && offset < range.end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> SyntaxEngine {
        SyntaxEngine::new()
    }

    fn path_for(language: SyntaxLanguage) -> &'static str {
        match language {
            SyntaxLanguage::Rust => "fixture.rs",
            SyntaxLanguage::C => "fixture.c",
            SyntaxLanguage::Cpp => "fixture.cpp",
            SyntaxLanguage::CSharp => "fixture.cs",
            SyntaxLanguage::Java => "fixture.java",
            SyntaxLanguage::Go => "fixture.go",
            SyntaxLanguage::Python => "fixture.py",
            SyntaxLanguage::JavaScript => "fixture.js",
            SyntaxLanguage::TypeScript => "fixture.ts",
            SyntaxLanguage::Html => "fixture.html",
            SyntaxLanguage::Css => "fixture.css",
            SyntaxLanguage::Json => "fixture.json",
            SyntaxLanguage::Toml => "fixture.toml",
            SyntaxLanguage::Yaml => "fixture.yaml",
            SyntaxLanguage::Markdown => "fixture.md",
            SyntaxLanguage::Bash => "fixture.sh",
        }
    }

    fn full_fixture(language: SyntaxLanguage) -> &'static str {
        match language {
            SyntaxLanguage::Rust => {
                include_str!("../../../tests/fixtures/syntax-engine/rust/full.txt")
            }
            SyntaxLanguage::C => include_str!("../../../tests/fixtures/syntax-engine/c/full.txt"),
            SyntaxLanguage::Cpp => {
                include_str!("../../../tests/fixtures/syntax-engine/cpp/full.txt")
            }
            SyntaxLanguage::CSharp => {
                include_str!("../../../tests/fixtures/syntax-engine/c_sharp/full.txt")
            }
            SyntaxLanguage::Java => {
                include_str!("../../../tests/fixtures/syntax-engine/java/full.txt")
            }
            SyntaxLanguage::Go => include_str!("../../../tests/fixtures/syntax-engine/go/full.txt"),
            SyntaxLanguage::Python => {
                include_str!("../../../tests/fixtures/syntax-engine/python/full.txt")
            }
            SyntaxLanguage::JavaScript => {
                include_str!("../../../tests/fixtures/syntax-engine/javascript/full.txt")
            }
            SyntaxLanguage::TypeScript => {
                include_str!("../../../tests/fixtures/syntax-engine/typescript/full.txt")
            }
            SyntaxLanguage::Html => {
                include_str!("../../../tests/fixtures/syntax-engine/html/full.txt")
            }
            SyntaxLanguage::Css => {
                include_str!("../../../tests/fixtures/syntax-engine/css/full.txt")
            }
            SyntaxLanguage::Json => {
                include_str!("../../../tests/fixtures/syntax-engine/json/full.txt")
            }
            SyntaxLanguage::Toml => {
                include_str!("../../../tests/fixtures/syntax-engine/toml/full.txt")
            }
            SyntaxLanguage::Yaml => {
                include_str!("../../../tests/fixtures/syntax-engine/yaml/full.txt")
            }
            SyntaxLanguage::Markdown => {
                include_str!("../../../tests/fixtures/syntax-engine/markdown/full.txt")
            }
            SyntaxLanguage::Bash => {
                include_str!("../../../tests/fixtures/syntax-engine/bash/full.txt")
            }
        }
    }

    fn incomplete_fixture(language: SyntaxLanguage) -> &'static str {
        match language {
            SyntaxLanguage::Rust => {
                include_str!("../../../tests/fixtures/syntax-engine/rust/incomplete.txt")
            }
            SyntaxLanguage::C => {
                include_str!("../../../tests/fixtures/syntax-engine/c/incomplete.txt")
            }
            SyntaxLanguage::Cpp => {
                include_str!("../../../tests/fixtures/syntax-engine/cpp/incomplete.txt")
            }
            SyntaxLanguage::CSharp => {
                include_str!("../../../tests/fixtures/syntax-engine/c_sharp/incomplete.txt")
            }
            SyntaxLanguage::Java => {
                include_str!("../../../tests/fixtures/syntax-engine/java/incomplete.txt")
            }
            SyntaxLanguage::Go => {
                include_str!("../../../tests/fixtures/syntax-engine/go/incomplete.txt")
            }
            SyntaxLanguage::Python => {
                include_str!("../../../tests/fixtures/syntax-engine/python/incomplete.txt")
            }
            SyntaxLanguage::JavaScript => {
                include_str!("../../../tests/fixtures/syntax-engine/javascript/incomplete.txt")
            }
            SyntaxLanguage::TypeScript => {
                include_str!("../../../tests/fixtures/syntax-engine/typescript/incomplete.txt")
            }
            SyntaxLanguage::Html => {
                include_str!("../../../tests/fixtures/syntax-engine/html/incomplete.txt")
            }
            SyntaxLanguage::Css => {
                include_str!("../../../tests/fixtures/syntax-engine/css/incomplete.txt")
            }
            SyntaxLanguage::Json => {
                include_str!("../../../tests/fixtures/syntax-engine/json/incomplete.txt")
            }
            SyntaxLanguage::Toml => {
                include_str!("../../../tests/fixtures/syntax-engine/toml/incomplete.txt")
            }
            SyntaxLanguage::Yaml => {
                include_str!("../../../tests/fixtures/syntax-engine/yaml/incomplete.txt")
            }
            SyntaxLanguage::Markdown => {
                include_str!("../../../tests/fixtures/syntax-engine/markdown/incomplete.txt")
            }
            SyntaxLanguage::Bash => {
                include_str!("../../../tests/fixtures/syntax-engine/bash/incomplete.txt")
            }
        }
    }

    #[test]
    fn all_languages_have_loadable_parsers() {
        for &language in SyntaxLanguage::all() {
            let parser = SyntaxEngine::parser_for(language);
            assert!(
                parser.language().is_some(),
                "missing parser for {}",
                language.name()
            );
        }
    }

    #[test]
    fn full_fixture_produces_structural_analysis_for_every_language() {
        for &language in SyntaxLanguage::all() {
            let mut engine = engine();
            let text = full_fixture(language);
            let update = engine.open_document(
                OpenDocument {
                    descriptor: DocumentDescriptor {
                        id: DocumentId(1),
                        version: 1,
                        large_file_mode: false,
                    },
                    path: Some(Path::new(path_for(language))),
                    language_override: Some(language),
                    text,
                },
                None,
            );
            assert_eq!(update.status, SyntaxStatus::Parsed, "{}", language.name());
            assert!(
                update.snapshot.highlights.iter().any(|span| matches!(
                    span.role,
                    StyleRole::SyntaxKeyword
                        | StyleRole::SyntaxString
                        | StyleRole::SyntaxComment
                        | StyleRole::SemanticType
                )),
                "{}",
                language.name()
            );
            assert!(!update.snapshot.symbols.is_empty(), "{}", language.name());
            assert!(!update.snapshot.folds.is_empty(), "{}", language.name());
            assert!(!update.snapshot.pairs.is_empty(), "{}", language.name());
        }
    }

    #[test]
    fn incomplete_fixture_keeps_partial_structure() {
        for &language in SyntaxLanguage::all() {
            let mut engine = engine();
            let text = incomplete_fixture(language);
            let update = engine.open_document(
                OpenDocument {
                    descriptor: DocumentDescriptor {
                        id: DocumentId(2),
                        version: 1,
                        large_file_mode: false,
                    },
                    path: Some(Path::new(path_for(language))),
                    language_override: Some(language),
                    text,
                },
                None,
            );
            assert_eq!(update.status, SyntaxStatus::Parsed, "{}", language.name());
            if !matches!(language, SyntaxLanguage::Html | SyntaxLanguage::Markdown) {
                assert!(
                    update.snapshot.has_error || !update.snapshot.error_regions.is_empty(),
                    "{}",
                    language.name()
                );
            }
        }
    }

    #[test]
    fn incremental_update_reuses_previous_tree() {
        let mut engine = engine();
        let first = include_str!("../../../tests/fixtures/syntax-engine/rust/full.txt");
        let second = first.replace("value = 1;", "value = 1 + 2;");
        let open = engine.open_document(
            OpenDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(3),
                    version: 1,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: first,
            },
            None,
        );
        assert_eq!(open.status, SyntaxStatus::Parsed);

        let update = engine.update_document(
            UpdateDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(3),
                    version: 2,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: &second,
            },
            None,
        );
        assert_eq!(update.status, SyntaxStatus::Parsed);
        assert!(!update.changed_ranges.is_empty());
    }

    #[test]
    fn stale_version_is_rejected() {
        let mut engine = engine();
        let open = engine.open_document(
            OpenDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(4),
                    version: 1,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: "fn main() {}",
            },
            None,
        );
        assert_eq!(open.status, SyntaxStatus::Parsed);

        let update = engine.update_document(
            UpdateDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(4),
                    version: 1,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: "fn main() { let value = 1; }",
            },
            None,
        );
        assert_eq!(update.status, SyntaxStatus::StaleVersion);
        let snapshot = engine.snapshot(DocumentId(4)).expect("snapshot");
        assert_eq!(snapshot.ticket.expect("ticket").version, 1);
    }

    #[test]
    fn cancellation_is_respected() {
        let mut engine = engine();
        let cancelled = || true;
        let update = engine.open_document(
            OpenDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(5),
                    version: 1,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: include_str!("../../../tests/fixtures/syntax-engine/rust/full.txt"),
            },
            Some(&cancelled),
        );
        assert_eq!(update.status, SyntaxStatus::Cancelled);
    }

    #[test]
    fn unicode_edits_are_parsed_incrementally() {
        let mut engine = engine();
        let first = "fn café() {\n    let greeting = \"héllo 😃\";\n}\n";
        let second = "fn café() {\n    let greeting = \"héllo 😄\";\n}\n";
        let open = engine.open_document(
            OpenDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(6),
                    version: 1,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: first,
            },
            None,
        );
        assert_eq!(open.status, SyntaxStatus::Parsed);

        let update = engine.update_document(
            UpdateDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(6),
                    version: 2,
                    large_file_mode: false,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: second,
            },
            None,
        );
        assert_eq!(update.status, SyntaxStatus::Parsed);
        assert!(!update.snapshot.highlights.is_empty());
    }

    #[test]
    fn large_file_mode_suppresses_parsing() {
        let mut engine = engine();
        let update = engine.open_document(
            OpenDocument {
                descriptor: DocumentDescriptor {
                    id: DocumentId(7),
                    version: 1,
                    large_file_mode: true,
                },
                path: Some(Path::new("fixture.rs")),
                language_override: Some(SyntaxLanguage::Rust),
                text: include_str!("../../../tests/fixtures/syntax-engine/rust/full.txt"),
            },
            None,
        );
        assert_eq!(update.status, SyntaxStatus::LargeFileSuppressed);
        assert!(update.snapshot.suppressed);
        assert!(update.snapshot.highlights.is_empty());
    }
}

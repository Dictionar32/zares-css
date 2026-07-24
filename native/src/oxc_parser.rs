//! Oxc AST parser untuk informasi struktural + regex untuk class extraction.
//!
//! Strategi hybrid yang proven:
//! - Oxc TSX pass: component names, imports, "use client"
//! - Regex (extract_classes_from_source): class extraction dari semua pola tw.*
//!
//! Ini menghindari keterbatasan Oxc 0.1.3 yang tidak bisa parse
//! JSX + tagged template literals bersamaan.

use once_cell::sync::Lazy;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{
    walk::{walk_import_declaration, walk_variable_declarator},
    Visit,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use regex::Regex;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Result type
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassOccurrence {
    pub class_name: String,
    /// 1-indexed line number, first occurrence of this class in the file.
    pub line: u32,
    /// 1-indexed column number, first occurrence of this class in the file.
    pub column: u32,
}

#[derive(Debug, Default)]
pub struct OxcExtractResult {
    pub classes: Vec<String>,
    pub component_names: Vec<String>,
    pub has_tw_usage: bool,
    pub has_use_client: bool,
    pub imports: Vec<String>,
    pub dynamic_props: Vec<DynamicPropUsage>,
    /// Non-empty when the Oxc parser failed to fully parse `source` (e.g. an
    /// ASI ambiguity like a bare statement ending without `;` followed by a
    /// line starting with `<`, which the parser can read as a `<` comparison
    /// operator instead of the start of JSX). When this is non-empty, every
    /// other field on this struct may be incomplete/empty for the affected
    /// file — the parse failed before the structural visitor ever ran.
    pub parse_errors: Vec<String>,
    /// First-occurrence (line, column) per unique class in `classes` — same
    /// dedup/filter/sort as `classes`, just paired with position. Powers
    /// `CssRule.source` population (see AGENT.md gap #4) so compiled CSS can
    /// carry a `/* file:line:column */` comment back to its origin.
    pub class_positions: Vec<ClassOccurrence>,
}

/// Classification of a JSX attribute's value, used to decide whether the
/// compiler can resolve it into a static class at build time, or whether it
/// must fall back to a runtime CSS mechanism (e.g. CSS custom property write).
///
/// - `Static`      → string/template literal with no expression → build-time class.
/// - `ThemeResolvable` → member expression rooted at an identifier that comes
///                       from a statically-importable module (e.g. `theme.primary`
///                       where `theme` is imported from a config file) → the
///                       compiler *may* be able to resolve this at build time,
///                       but needs to actually load and inspect that module —
///                       this variant just flags "worth attempting," it is not
///                       a guarantee of success.
/// - `Runtime`     → anything else (identifiers bound to local state/props,
///                   call expressions, conditional expressions, etc.) → cannot
///                   be known at build time, must be written to CSS at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropValueKind {
    Static,
    ThemeResolvable { root: String },
    Runtime,
}

#[derive(Debug, Clone)]
pub struct DynamicPropUsage {
    pub component_name: String,
    pub attr_name: String,
    pub kind: PropValueKind,
}

// ─────────────────────────────────────────────────────────────────────────────
// Visitor struktural (TSX pass) — hanya ambil nama komponen + imports
// ─────────────────────────────────────────────────────────────────────────────

struct StructuralVisitor {
    component_names: Vec<String>,
    imports: Vec<String>,
    has_use_client: bool,
    dynamic_props: Vec<DynamicPropUsage>,
    // Names bound by an `import ... from "..."` where the source path looks
    // like a theme/config module. Heuristic only (see classify_expression) —
    // real resolution still requires the compiler to load the module.
    theme_like_imports: std::collections::HashSet<String>,
    // Name of the JSX-returning function component currently being visited,
    // used only to label dynamic_props entries for easier debugging/reporting.
    current_component: Vec<String>,
}

impl StructuralVisitor {
    fn new() -> Self {
        Self {
            component_names: Vec::new(),
            imports: Vec::new(),
            has_use_client: false,
            dynamic_props: Vec::new(),
            theme_like_imports: std::collections::HashSet::new(),
            current_component: Vec::new(),
        }
    }

    /// Walks a (possibly nested) expression down to its root identifier,
    /// e.g. `theme.colors.primary` → "theme".
    fn root_identifier(expr: &Expression) -> Option<String> {
        match expr.get_inner_expression() {
            Expression::Identifier(id) => Some(id.name.to_string()),
            expr if expr.is_member_expression() => {
                Self::root_identifier(expr.to_member_expression().object())
            }
            _ => None,
        }
    }

    fn classify_expression(&self, expr: &Expression) -> PropValueKind {
        match expr.get_inner_expression() {
            Expression::StringLiteral(_)
            | Expression::TemplateLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_) => PropValueKind::Static,

            expr if expr.is_member_expression() => {
                match Self::root_identifier(expr) {
                    Some(root) if self.theme_like_imports.contains(&root) => {
                        PropValueKind::ThemeResolvable { root }
                    }
                    _ => PropValueKind::Runtime,
                }
            }

            // Bare identifier referencing something other than a known
            // theme import (local state, destructured props, etc.) — no way
            // to know its value without full scope/data-flow analysis.
            Expression::Identifier(id) => {
                if self.theme_like_imports.contains(id.name.as_str()) {
                    // `bgColor={theme}` directly, unlikely but handle it
                    PropValueKind::ThemeResolvable {
                        root: id.name.to_string(),
                    }
                } else {
                    PropValueKind::Runtime
                }
            }

            // Calls, conditionals, binary expressions, arrow functions, etc.
            _ => PropValueKind::Runtime,
        }
    }

    /// Heuristic: treat an import source as "theme-like" if its path
    /// suggests a config/theme/token module rather than an arbitrary
    /// component or utility import. This intentionally over-includes —
    /// false positives just mean the compiler *attempts* build-time
    /// resolution and falls back to runtime if the module doesn't actually
    /// contain the referenced value.
    fn is_theme_like_source(source: &str) -> bool {
        let lower = source.to_lowercase();
        lower.contains("theme") || lower.contains("tokens") || lower.contains("design-system")
    }

    fn is_tw(expr: &Expression) -> bool {
        match expr.get_inner_expression() {
            // "server" is the RSC-only sibling of "tw" (same object-config /
            // template-literal call surface, e.g. `server.div({...})`) — see
            // RE_TEMPLATE/RE_COMP_NAME in transform.rs which already special-case
            // `tw.server.*`. Recognizing the bare `server` root here keeps the
            // AST-based component-name pass consistent with that.
            Expression::Identifier(id) => id.name == "tw" || id.name == "server",
            expr if expr.is_member_expression() => {
                Self::is_tw(expr.to_member_expression().object())
            }
            Expression::CallExpression(ce) => Self::is_tw(&ce.callee),
            Expression::ChainExpression(chain) => chain
                .expression
                .as_member_expression()
                .is_some_and(|me| Self::is_tw(me.object())),
            _ => false,
        }
    }
}

impl<'a> Visit<'a> for StructuralVisitor {
    fn visit_directive(&mut self, dir: &Directive<'a>) {
        if dir.expression.value == "use client" {
            self.has_use_client = true;
        }
    }

    fn visit_import_declaration(&mut self, decl: &ImportDeclaration<'a>) {
        let source = decl.source.value.to_string();
        if Self::is_theme_like_source(&source) {
            if let Some(specifiers) = &decl.specifiers {
                for spec in specifiers {
                    // Cover default, named, and namespace imports — any of
                    // `import theme from`, `import { theme } from`,
                    // `import * as theme from` bound name gets tracked.
                    let local_name = match spec {
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            Some(s.local.name.to_string())
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(s) => {
                            Some(s.local.name.to_string())
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            Some(s.local.name.to_string())
                        }
                    };
                    if let Some(name) = local_name {
                        self.theme_like_imports.insert(name);
                    }
                }
            }
        }
        self.imports.push(source);
        walk_import_declaration(self, decl);
    }

    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        let component_label = self
            .current_component
            .last()
            .cloned()
            .unwrap_or_else(|| "<unknown>".to_string());

        for attr_item in &elem.attributes {
            let JSXAttributeItem::Attribute(attr) = attr_item else {
                // Spread attributes ({...props}) can't be classified statically —
                // deliberately not treated as a hard Runtime entry here since
                // there's no single attr_name to attach it to.
                continue;
            };

            let JSXAttributeName::Identifier(name_ident) = &attr.name else {
                continue; // namespaced attrs (rare) — skip
            };
            let attr_name = name_ident.name.to_string();

            let Some(value) = &attr.value else {
                continue; // boolean attribute, e.g. `disabled` — nothing to classify
            };

            let kind = match value {
                JSXAttributeValue::StringLiteral(_) => PropValueKind::Static,
                JSXAttributeValue::ExpressionContainer(container) => {
                    match &container.expression {
                        JSXExpression::EmptyExpression(_) => continue,
                        expr => self.classify_expression(expr.to_expression()),
                    }
                }
                // JSXElement / JSXFragment as attribute value — treat as
                // runtime; classifying nested JSX trees is out of scope here.
                _ => PropValueKind::Runtime,
            };

            self.dynamic_props.push(DynamicPropUsage {
                component_name: component_label.clone(),
                attr_name,
                kind,
            });
        }

        oxc_ast_visit::walk::walk_jsx_opening_element(self, elem);
    }

    fn visit_function(&mut self, func: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        // Heuristic: PascalCase named function = likely a component.
        let is_component_name = func
            .id
            .as_ref()
            .map(|id| id.name.chars().next().is_some_and(|c| c.is_uppercase()))
            .unwrap_or(false);

        if is_component_name {
            self.current_component
                .push(func.id.as_ref().unwrap().name.to_string());
        }

        oxc_ast_visit::walk::walk_function(self, func, flags);

        if is_component_name {
            self.current_component.pop();
        }
    }

    fn visit_variable_declarator(&mut self, decl: &VariableDeclarator<'a>) {
        if let BindingPatternKind::BindingIdentifier(id) = &decl.id.kind {
            if let Some(init) = &decl.init {
                let is_tw = matches!(init,
                    Expression::TaggedTemplateExpression(t) if Self::is_tw(&t.tag)
                ) || matches!(init,
                    Expression::CallExpression(c) if Self::is_tw(&c.callee)
                );
                if is_tw {
                    self.component_names.push(id.name.to_string());
                }
            }
        }
        walk_variable_declarator(self, decl);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Regex-based class extractor (proven reliable, handles mixed JSX + templates)
// ─────────────────────────────────────────────────────────────────────────────

/// Regex patterns untuk semua cara kelas Tailwind bisa ditulis
static RE_TW_TEMPLATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)\btw(?:\.server)?\.(?:\w+)`([^`]*)`").unwrap());
static RE_TW_WRAP: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)\btw\(\w+\)`([^`]*)`").unwrap());
static RE_BASE_FIELD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"base\s*:\s*["'`]([^"'`]+)["'`]"#).unwrap());
static RE_VARIANTS_LEAF: Lazy<Regex> = Lazy::new(|| {
    // Match any key: "value" pattern in variant objects — filtered by is_tw_class downstream
    Regex::new(r#"\w+\s*:\s*["'`]([^"'`]+)["'`]"#).unwrap()
});
static RE_CLASSNAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:className|class)=["']([^"']+)["']"#).unwrap());
// Tangkap SEMUA string literals di dalam cx/cn/clsx/twMerge call (multi-arg)
static RE_CX_CALL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\b(?:cx|cn|clsx|classnames|twMerge)\([^)]+\)"#).unwrap());
// Sub-pattern untuk ekstrak setiap string literal di dalam call args
static RE_STRING_ARG: Lazy<Regex> = Lazy::new(|| Regex::new(r#"["']([^"']+)["']"#).unwrap());

/// Splits `s` on whitespace like `str::split_whitespace()`, but also yields
/// each token's byte offset within `s`. Used to compute absolute source
/// positions for classes found inside a larger regex capture group (e.g.
/// `className="flex items-center"` — need the offset of "items-center"
/// specifically, not just the whole attribute).
fn split_whitespace_with_offsets(s: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut idx = 0;
    s.split_whitespace().map(move |tok| {
        // Safe: tokens from split_whitespace() on `s` always appear in order
        // and don't overlap, so searching from the last end position always
        // finds the correct occurrence of `tok`, not an earlier coincidental
        // match of the same substring.
        let start = s[idx..].find(tok).map(|p| p + idx).unwrap_or(idx);
        idx = start + tok.len();
        (start, tok)
    })
}

/// Converts a byte offset into 1-indexed (line, column) against `source`.
/// Column counted in UTF-8 bytes since last newline + 1 — close enough for
/// ASCII/most source; not UTF-16/grapheme-exact like an editor LSP.
fn line_col_at(source: &str, byte_offset: usize) -> (u32, u32) {
    let offset = byte_offset.min(source.len());
    let mut line: u32 = 1;
    let mut last_newline = 0usize;
    for (i, b) in source.as_bytes()[..offset].iter().enumerate() {
        if *b == b'\n' {
            line += 1;
            last_newline = i + 1;
        }
    }
    (line, (offset - last_newline + 1) as u32)
}

pub(crate) fn extract_classes_regex(source: &str) -> Vec<String> {
    extract_classes_regex_with_positions(source)
        .into_iter()
        .map(|(class, _, _)| class)
        .collect()
}

/// Same extraction as `extract_classes_regex`, but also returns each class's
/// 1-indexed (line, column) — the FIRST occurrence's position if a class
/// string appears more than once in the file. Used to populate
/// `CssRule.source` when compiling (see `compileClassesWithSource` on the
/// TS side / `AGENT.md` gap #4).
pub(crate) fn extract_classes_regex_with_positions(source: &str) -> Vec<(String, u32, u32)> {
    let mut raw: Vec<(String, u32, u32)> = Vec::new();

    let mut push = |raw: &mut Vec<(String, u32, u32)>, content: &str, group_start: usize| {
        for (rel_offset, t) in split_whitespace_with_offsets(content) {
            let t = t.trim();
            if !t.is_empty() && !t.ends_with('{') && t != "}" {
                let (line, col) = line_col_at(source, group_start + rel_offset);
                raw.push((t.to_string(), line, col));
            }
        }
    };

    // tw.div`classes` dan tw.server.div`classes`
    for cap in RE_TW_TEMPLATE.captures_iter(source) {
        let group = cap.get(1).unwrap();
        let content = group.as_str();
        // Skip dynamic (${...})
        if !content.contains("${") {
            push(&mut raw, content, group.start());
        }
    }

    // tw(Component)`classes`
    for cap in RE_TW_WRAP.captures_iter(source) {
        let group = cap.get(1).unwrap();
        let content = group.as_str();
        if !content.contains("${") {
            push(&mut raw, content, group.start());
        }
    }

    // base: "classes"
    for cap in RE_BASE_FIELD.captures_iter(source) {
        let group = cap.get(1).unwrap();
        push(&mut raw, group.as_str(), group.start());
    }

    // variant leaf values (heuristic — ambil string pendek di dalam objek)
    for cap in RE_VARIANTS_LEAF.captures_iter(source) {
        let group = cap.get(1).unwrap();
        let val = group.as_str();
        // Hanya ambil jika terlihat seperti kumpulan kelas Tailwind
        if val.len() < 200 && (val.contains('-') || val.contains(':')) {
            push(&mut raw, val, group.start());
        }
    }

    // className="classes" dan class="classes"
    for cap in RE_CLASSNAME.captures_iter(source) {
        let group = cap.get(1).unwrap();
        push(&mut raw, group.as_str(), group.start());
    }

    // cx("a", "b") / cn("a", "b") / clsx("a", "b") / twMerge("a", "b")
    // RE_CX_CALL menangkap seluruh call, RE_STRING_ARG ekstrak tiap string arg
    for call_cap in RE_CX_CALL.captures_iter(source) {
        let call_group = call_cap.get(0).unwrap();
        let call_text = call_group.as_str();
        for str_cap in RE_STRING_ARG.captures_iter(call_text) {
            let str_group = str_cap.get(1).unwrap();
            // Offset relatif ke `call_text`, harus ditambah offset awal call
            // di `source` biar jadi absolute.
            push(&mut raw, str_group.as_str(), call_group.start() + str_group.start());
        }
    }

    raw
}

// ─────────────────────────────────────────────────────────────────────────────
// Filter Tailwind class
// ─────────────────────────────────────────────────────────────────────────────

fn is_tw_class(c: &str) -> bool {
    // Kelas dengan separator selalu Tailwind
    if c.contains('-') || c.contains(':') || c.contains('[') {
        return true;
    }
    // Single-word Tailwind utilities yang valid
    matches!(
        c,
        // Layout
        "flex" | "grid" | "block" | "inline" | "hidden" | "contents" | "table" |
        "static" | "fixed" | "absolute" | "relative" | "sticky" |
        "overflow" | "truncate" | "container" | "float" | "clear" |
        // Typography
        "italic" | "underline" | "uppercase" | "lowercase" | "capitalize" |
        "overline" | "antialiased" | "subpixel" | "ordinal" | "slashed" |
        // Visibility
        "visible" | "invisible" | "collapse" | "prose" |
        // Flexbox / Grid
        "grow" | "shrink" | "wrap" | "nowrap" |
        // Borders
        "rounded" | "border" | "outline" | "ring" | "shadow" | "divide" |
        // Interactivity
        "cursor" | "pointer" | "select" | "resize" |
        // Spacing
        "space" |
        // Colors (single word used as modifier)
        "transparent" | "current" | "inherit" |
        // Misc
        "transform" | "transition" | "animate" | "appearance" | "placeholder" |
        "sr" | "not" | "peer" | "group" | "dark" | "motion"
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic formatting — TypeScript-style `file: message` (per parse error)
// ─────────────────────────────────────────────────────────────────────────────

/// Formats every parse error from an Oxc `ParserReturn` into a readable,
/// TypeScript-like line: `filename - error: <message>`.
///
/// NOTE: this intentionally only relies on `OxcDiagnostic`'s `Display` impl
/// (`{}`), not on internal fields like `.labels`/`.message`/`.span()` —
/// those aren't guaranteed to be public across Oxc versions, and guessing
/// wrong here just trades one compile error for another. `Display` already
/// includes Oxc's own message text; if you want the fully rendered
/// miette-style report with source-code context and a `^^^` pointer under
/// the offending token, pipe `errors` through `miette::Report::new(err)
/// .with_source_code(source.to_string())` and format that with `{:?}`
/// instead — left out here since the exact `with_source_code` signature
/// should be confirmed against the `oxc_diagnostics`/`miette` version this
/// crate pins before relying on it.
fn format_parse_errors<E: std::fmt::Display>(errors: &[E], filename: &str) -> Vec<String> {
    errors
        .iter()
        .map(|err| format!("{filename} - error: {err}"))
        .collect()
}



pub(crate) fn run_structural_pass(
    source: &str,
    filename: &str,
) -> (Vec<String>, bool, Vec<String>, Vec<DynamicPropUsage>, Vec<String>) {
    // NOTE: previously this stripped standalone JSX lines via regex before parsing,
    // on the assumption that Oxc 0.1.3 could not parse JSX + tagged template
    // literals in the same file. Verified empirically (2026-07) against a modern
    // Oxc release that this limitation no longer applies — JSX and tagged
    // template literals parse together without error. The strip step has been
    // removed so the visitor now sees real JSX nodes (JSXElement,
    // JSXAttribute, JSXExpressionContainer, etc.) instead of having them
    // deleted before the tree is built.
    //
    // Removing the strip DOES expose a real (pre-existing) ASI hazard: a
    // top-level statement without a trailing `;` immediately followed by a
    // line starting with `<` can be misread as a `<` comparison operator
    // instead of the start of a new JSX statement, which fails the whole
    // parse for that file. `ret.errors` is checked below so that failure is
    // surfaced instead of silently returning empty structural data.
    let allocator = Allocator::default();
    let st = SourceType::from_path(Path::new("file.tsx"))
        .unwrap_or_default()
        .with_module(true);
    let ret = Parser::new(&allocator, source, st).parse();

    let parse_errors = format_parse_errors(&ret.errors, filename);

    let mut v = StructuralVisitor::new();
    // SAFETY: All data extracted by the visitor is owned String,
    // no references to the AST escape this function.
    // The allocator lives alongside ret, so the borrow is valid for the function scope.
    let prog: &Program = unsafe { &*(&ret.program as *const Program) };
    v.visit_program(prog);
    drop(ret);

    (
        v.component_names,
        v.has_use_client,
        v.imports,
        v.dynamic_props,
        parse_errors,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

pub fn extract_classes_oxc(source: &str, filename: &str) -> OxcExtractResult {
    // Pass 1: AST struktural (Oxc TSX) — nama komponen, imports, "use client",
    // klasifikasi dynamic prop values (static / theme-resolvable / runtime),
    // dan parse errors (kalau ada — lihat catatan ASI hazard di run_structural_pass)
    let (component_names, has_use_client, imports, dynamic_props, parse_errors) =
        run_structural_pass(source, filename);

    if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!(
                "[zares-css] oxc parse error — structural extraction (component names, \
                 imports, dynamic props) is likely INCOMPLETE for this file: {err}"
            );
        }
    }

    // Pass 2: Regex class extraction — proven, handles mixed JSX + templates
    let raw_classes_with_pos = extract_classes_regex_with_positions(source);

    // Text-level has_tw_usage detection
    let has_tw_usage = source.contains("tw.")
        || source.contains("from \"tailwind-styled")
        || source.contains("from 'tailwind-styled");

    // Dedup + filter — keep FIRST occurrence's position per unique class.
    let mut seen = std::collections::HashSet::new();
    let mut classes: Vec<String> = Vec::new();
    let mut class_positions: Vec<ClassOccurrence> = Vec::new();
    for (class_name, line, column) in raw_classes_with_pos {
        if is_tw_class(&class_name) && seen.insert(class_name.clone()) {
            class_positions.push(ClassOccurrence {
                class_name: class_name.clone(),
                line,
                column,
            });
            classes.push(class_name);
        }
    }
    classes.sort();
    // `class_positions` intentionally NOT sorted the same way — kept in
    // source-appearance order, which is more useful for "where did this
    // come from" lookups than alphabetical order.

    OxcExtractResult {
        classes,
        component_names,
        has_tw_usage,
        has_use_client,
        imports,
        dynamic_props,
        parse_errors,
        class_positions,
    }
}
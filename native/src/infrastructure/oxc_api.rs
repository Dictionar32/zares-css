use napi_derive::napi;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::oxc_parser::{self, PropValueKind};

/// NAPI-friendly flattening of `oxc_parser::DynamicPropUsage`. Rust enums
/// carrying data (`PropValueKind::ThemeResolvable { root }`) don't map
/// cleanly across the NAPI boundary, so `kind` is exposed as a plain string
/// discriminant and `theme_root` carries the payload only when relevant.
///
/// `kind` is one of: "static" | "theme_resolvable" | "runtime".
#[napi(object)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OxcDynamicPropUsage {
    pub component_name: String,
    pub attr_name: String,
    pub kind: String,
    /// Only present when `kind === "theme_resolvable"` — the root import
    /// identifier the value was traced back to (e.g. "theme").
    pub theme_root: Option<String>,
}

impl From<oxc_parser::DynamicPropUsage> for OxcDynamicPropUsage {
    fn from(u: oxc_parser::DynamicPropUsage) -> Self {
        let (kind, theme_root) = match u.kind {
            PropValueKind::Static => ("static".to_string(), None),
            PropValueKind::Runtime => ("runtime".to_string(), None),
            PropValueKind::ThemeResolvable { root } => ("theme_resolvable".to_string(), Some(root)),
        };
        Self {
            component_name: u.component_name,
            attr_name: u.attr_name,
            kind,
            theme_root,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OxcExtractResult {
    pub classes: Vec<String>,
    pub component_names: Vec<String>,
    pub has_tw_usage: bool,
    pub has_use_client: bool,
    pub imports: Vec<String>,
    pub engine: String,
    pub dynamic_props: Vec<OxcDynamicPropUsage>,
    /// Non-empty when Oxc failed to fully parse `source` — see
    /// `oxc_parser::OxcExtractResult::parse_errors` for what this implies
    /// about the other fields on this struct (they may be incomplete).
    pub parse_errors: Vec<String>,
}

/// Extract Tailwind classes using real Oxc AST parser.
/// Handles: tw.tag``, tw(Comp)``, base:"", className="", cx()/cn()
/// More accurate than regex — understands JSX, TypeScript, template literals.
#[napi]
pub fn oxc_extract_classes(source: String, filename: String) -> OxcExtractResult {
    let r = oxc_parser::extract_classes_oxc(&source, &filename);
    OxcExtractResult {
        classes: r.classes,
        component_names: r.component_names,
        has_tw_usage: r.has_tw_usage,
        has_use_client: r.has_use_client,
        imports: r.imports,
        engine: "oxc".to_string(),
        dynamic_props: r.dynamic_props.into_iter().map(Into::into).collect(),
        parse_errors: r.parse_errors,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
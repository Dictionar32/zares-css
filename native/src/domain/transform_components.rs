use napi_derive::napi;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::domain::transform::SubComponent;
use crate::shared::utils::{serde_json_string, short_hash};

// ─────────────────────────────────────────────────────────────────────────────
// BUG FIX: RE_BLOCK lama pakai \b word boundary sehingga tidak bisa match
// `[icon] { ... }` karena '[' bukan word character.
// Akibatnya seluruh isi [icon] block bocor masuk ke base button className
// dan twMerge menerima h-12 vs h-4, flex vs flex, dst sebagai satu flat list.
//
// Fix: ganti dengan dua regex terpisah:
//   RE_BLOCK_BRACKET — canonical syntax `[name] { ... }` (tidak ada ambiguitas
//                       dengan Tailwind arbitrary values seperti bg-[#383838]
//                       karena arbitrary values tidak memiliki closing `}`)
//   RE_BLOCK_BARE    — legacy compat `name { ... }` dengan \b agar aman
// ─────────────────────────────────────────────────────────────────────────────

static RE_BLOCK_BRACKET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[([a-z][a-zA-Z0-9_-]*)\]\s*\{([^}]*)\}").unwrap());

static RE_BLOCK_BARE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)\b([a-z][a-zA-Z0-9_]*)\s*\{([^}]*)\}").unwrap());

pub(crate) fn parse_subcomponent_blocks(
    template: &str,
    component_name: &str,
) -> (String, Vec<SubComponent>) {
    let mut sub_components: Vec<SubComponent> = Vec::new();
    let mut stripped = template.to_string();
    let mut matches: Vec<(String, String, String)> = Vec::new();

    // Pass 1: bracket syntax `[name] { ... }` — primary
    for c in RE_BLOCK_BRACKET.captures_iter(template) {
        matches.push((c[0].to_string(), c[1].to_string(), c[2].to_string()));
    }

    // Pass 2: bare syntax `name { ... }` — legacy compat, run pada residual
    let after_bracket = RE_BLOCK_BRACKET.replace_all(template, "");
    for c in RE_BLOCK_BARE.captures_iter(&after_bracket) {
        let sub_name = c[1].to_string();
        let already = matches.iter().any(|(_, n, _)| n == &sub_name);
        if !already {
            matches.push((c[0].to_string(), sub_name, c[2].to_string()));
        }
    }

    for (full_match, sub_name, sub_classes_raw) in &matches {
        let sub_classes = sub_classes_raw.trim().to_string();
        if sub_classes.is_empty() {
            continue;
        }

        let sub_tag = match sub_name.as_str() {
            "label" => "label",
            "input" => "input",
            "img" | "image" => "img",
            "header" => "header",
            "footer" => "footer",
            "icon" => "span",
            "button" => "button",
            "a" | "link" => "a",
            _ => "span",
        };

        let hash_input = format!("{}_{}_{}", component_name, sub_name, sub_classes);
        let hash = short_hash(&hash_input);
        let scoped_class = format!("{}_{}_{}", component_name, sub_name, hash);

        sub_components.push(SubComponent {
            name: sub_name.clone(),
            tag: sub_tag.to_string(),
            classes: sub_classes.clone(),
            scoped_class,
        });

        stripped = stripped.replace(full_match.as_str(), "");
    }

    (stripped.trim().to_string(), sub_components)
}

// ─────────────────────────────────────────────────────────────────────────────
// Component code generators
// ─────────────────────────────────────────────────────────────────────────────

/// `dynamic_props`: `(source_prop_name, css_var_name)` pairs collected while
/// resolving `${...}` tokens (see `transform::resolve_dynamic_classes`).
/// When non-empty, the generated component destructures those prop names and
/// sets them as CSS custom properties on the root element's `style` — merged
/// with any `style` the caller already passed in. CSS custom properties
/// inherit down the DOM tree, so a value set here also reaches sub-components
/// rendered as descendants (see README §3.5 "Approach 3").
pub(crate) fn render_static_component(
    tag: &str,
    classes: &str,
    fn_name: &str,
    with_sub: bool,
    dynamic_props: &[(String, String)],
) -> String {
    let dyn_style_line = dynamic_style_assign_line(dynamic_props);
    let delete_dyn_props = delete_dynamic_props_line(dynamic_props);
    let style_field = if dynamic_props.is_empty() {
        String::new()
    } else {
        ", style: _st".to_string()
    };
    let base = format!(
        "React.forwardRef(function {fn_name}(props, ref) {{\n  var _c = props.className;\n{dyn_style_line}  var _r = Object.assign({{}}, props);\n  delete _r.className;{delete_dyn_props}\n  return React.createElement(\"{tag}\", Object.assign({{ ref }}, _r, {{ className: [{classes_json}, _c].filter(Boolean).join(\" \"){style_field} }}));\n}})",
        fn_name = fn_name,
        tag = tag,
        classes_json = serde_json_string(classes),
    );
    // .withSub<...>() runtime adalah no-op (() => component). Attach method ke
    // forwardRef result via IIFE — source-nya tetap bisa .withSub() call,
    // hasilnya identik secara runtime tapi gak butuh parseTemplate native.
    if with_sub {
        format!(
            "(function(){{ var _c = {base}; _c.withSub = function(){{ return _c; }}; return _c; }})()",
            base = base,
        )
    } else {
        base
    }
}

/// Emit `var _ds = { "--Card-bg": props.bg, ... }; var _st = Object.assign({}, props.style, _ds);`
/// React omits `undefined`/`null` entries when applying a style object, so
/// props the caller didn't pass simply fall through to the CSS `var(..., fallback)`.
pub(crate) fn dynamic_style_assign_line(dynamic_props: &[(String, String)]) -> String {
    if dynamic_props.is_empty() {
        return String::new();
    }
    let entries = dynamic_props
        .iter()
        .map(|(source, var_name)| format!("{}: props.{}", serde_json_string(var_name), source))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "  var _ds = {{ {entries} }};\n  var _st = Object.assign({{}}, props.style, _ds);\n",
        entries = entries
    )
}

/// Emit ` delete _r.style; delete _r.bg; delete _r.color;` etc, so dynamic prop
/// names don't leak onto the underlying DOM element as unknown attributes.
pub(crate) fn delete_dynamic_props_line(dynamic_props: &[(String, String)]) -> String {
    if dynamic_props.is_empty() {
        return String::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = String::from(" delete _r.style;");
    for (source, _) in dynamic_props {
        if seen.insert(source.clone()) {
            out.push_str(&format!(" delete _r.{};", source));
        }
    }
    out
}

/// `dynamic_props`: applied to the root (`base`) element only — sub-components
/// are rendered as DOM descendants when nested in JSX (`<Card><Card.header/></Card>`),
/// so CSS custom property inheritance carries the value down without needing
/// each sub-component to also destructure it. See `render_static_component`.
pub(crate) fn render_compound_component(
    tag: &str,
    base_classes: &str,
    fn_name: &str,
    sub_components: &[SubComponent],
    component_name: &str,
    with_sub: bool,
    dynamic_props: &[(String, String)],
) -> String {
    let dyn_style_line = dynamic_style_assign_line(dynamic_props);
    let delete_dyn_props = delete_dynamic_props_line(dynamic_props);
    let style_field = if dynamic_props.is_empty() {
        String::new()
    } else {
        ", style: _st".to_string()
    };
    let base = format!(
        "React.forwardRef(function {fn_name}(props, ref) {{\n  var _c = props.className;\n{dyn_style_line}  var _r = Object.assign({{}}, props);\n  delete _r.className;{delete_dyn_props}\n  return React.createElement(\"{tag}\", Object.assign({{ ref }}, _r, {{ className: [{base_json}, _c].filter(Boolean).join(\" \"){style_field} }}));\n}})",
        fn_name = fn_name,
        tag = tag,
        base_json = serde_json_string(base_classes),
    );

    if sub_components.is_empty() {
        // .withSub<>() tanpa sub-block syntax di template — attach method ke
        // forwardRef result via IIFE sama seperti render_static_component.
        if with_sub {
            return format!(
                "(function(){{ var _c = {base}; _c.withSub = function(){{ return _c; }}; return _c; }})()",
                base = base,
            );
        }
        return base;
    }

    // ─ OPTIMIZATION (Phase 1.1): Pre-allocate sub_assignments vector
    let mut sub_assignments: Vec<String> = Vec::with_capacity(sub_components.len());
    for sub in sub_components {
        let sub_fn = format!("_Tw_{}_{}", component_name, sub.name);
        // BUG FIX: gunakan sub.classes (raw Tailwind classes) bukan sub.scoped_class.
        //
        // scoped_class ("Button_icon_195821") tidak punya CSS yang di-generate —
        // Tailwind hanya generate CSS untuk utility classes yang muncul di source.
        // registerFileClasses() adalah no-op, metadata_json tidak di-consume loader.
        // Akibatnya icon element punya class tapi TANPA styles apapun.
        //
        // Solusi: emit raw Tailwind classes langsung ke className, sama persis
        // seperti runtime path di twProxy.ts:
        //   className: className ? `${classes} ${className}` : classes
        // Tailwind content scan akan pickup classes ini dari compiled output.
        sub_assignments.push(format!(
            "  _base.{sub_name} = React.forwardRef(function {sub_fn}(props, ref) {{\n    var _c = props.className;\n    var _r = Object.assign({{}}, props);\n    delete _r.className;\n    return React.createElement(\"{tag}\", Object.assign({{ ref }}, _r, {{ className: [{classes_json}, _c].filter(Boolean).join(\" \") }}));\n  }});",
            sub_name = sub.name,
            sub_fn = sub_fn,
            tag = sub.tag,
            classes_json = serde_json_string(&sub.classes),
        ));
    }

    let iife = format!(
        "(function() {{\n  var _base = {base};\n{subs}\n  return _base;\n}})()",
        base = base,
        subs = sub_assignments.join("\n"),
    );
    // Jika .withSub<>() chained, method sudah ter-assign di _base dari sub-assignments
    // loop di atas (setiap sub: _base.logo, _base.links, dst). Tapi withSub() method
    // sendiri juga perlu ada di _base agar call `.withSub()` setelah IIFE tidak throw.
    if with_sub {
        format!(
            "(function(){{ var _c = {iife}; _c.withSub = function(){{ return _c; }}; return _c; }})()",
            iife = iife,
        )
    } else {
        iife
    }
}

pub(crate) fn build_metadata_json(
    component_name: &str,
    tag: &str,
    base_class: &str,
    sub_components: &[SubComponent],
) -> String {
    // ─ OPTIMIZATION: Use serde_json for correct escaping + no manual format! strings
    use serde_json::{json, Value};

    let subs: serde_json::Map<String, Value> = sub_components
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                json!({ "tag": s.tag, "class": s.scoped_class }),
            )
        })
        .collect();

    let meta = json!({
        "component": component_name,
        "tag": tag,
        "baseClass": base_class,
        "subComponents": subs,
    });

    serde_json::to_string(&meta)
        .unwrap_or_else(|_| format!("{{\"component\":\"{component_name}\"}}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// N-API exports
// ─────────────────────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// NAPI export — wraps pub(crate) parse_subcomponent_blocks for TS consumers
// ─────────────────────────────────────────────────────────────────────────────

/// Result type untuk parse_subcomponent_blocks_napi
#[napi(object)]
pub struct SubcomponentParseResult {
    /// Base template string dengan semua sub-component blocks dihapus
    pub base_classes: String,
    /// JSON string dari HashMap<name, classes>: {"icon":"h-4 w-4","badge":"px-2"}
    pub sub_map_json: String,
}

/// Parse sub-component blocks dari template literal.
/// Menggantikan JS regex parser di createComponent.ts.
///
/// # Example
/// ```ts
/// const r = native.parseSubcomponentBlocksNapi("p-4 [icon] { h-4 w-4 } flex", "tw")
/// // r.baseClasses = "p-4 flex"
/// // r.subMapJson  = "{\"icon\":\"h-4 w-4\"}"
/// ```
#[napi]
pub fn parse_subcomponent_blocks_napi(
    template: String,
    component_name: String,
) -> SubcomponentParseResult {
    let (base, subs) = parse_subcomponent_blocks(&template, &component_name);

    // Build JSON manually — avoid serde dependency
    let mut pairs: Vec<String> = Vec::with_capacity(subs.len());
    for s in &subs {
        let k = s.name.replace('\\', "\\\\").replace('"', "\\\"");
        let v = s.classes.replace('\\', "\\\\").replace('"', "\\\"");
        pairs.push(format!("\"{}\":\"{}\"", k, v));
    }
    let sub_map_json = format!("{{{}}}", pairs.join(","));

    SubcomponentParseResult {
        base_classes: base,
        sub_map_json,
    }
}
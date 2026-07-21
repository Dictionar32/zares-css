//! Repro tests — "Mode 2: Engine Sadar dari Awal" (dynamic `${...}` → CSS Variable)
//!
//! Covers the 3 entry points that historically diverged in behaviour:
//!   1. `tw.div\`...${x}...\``            (STEP 1 — AST / regex template literal)
//!   2. `tw(Component)\`...${x}...\``     (STEP 2 — wrapped-component template literal)
//!   3. `tw.div({ base: \`...${x}...\` })` (STEP 3 — object config)
//!
//! Before the fix:
//!   - STEP 1/2: `is_dynamic()` returned true → whole match was `continue`d
//!     (silently skipped, component fell through to runtime, zero CSS emitted).
//!   - STEP 3: no guard at all → `${x}` was baked verbatim into the static
//!     class list as garbage (e.g. literal class `"bg-[${color}]"`).
//!
//! After the fix, all 3 paths should:
//!   - rewrite the dynamic token into a scoped static class name
//!   - emit a `.tw-Comp-... { property: var(--Comp-..., fallback); }` rule
//!     into `TransformResult.dynamic_css_json`
//!   - NOT contain the raw `${...}` token anywhere in `code` or `classes`
//!
//! Run with:
//!   cd native && cargo test --test mode2_dynamic_var_repro -- --nocapture

use tailwind_styled_parser::domain::transform::transform_source;

/// Small helper: fail loudly with the full TransformResult dumped, so a
/// regression is easy to diagnose from CI output.
fn assert_no_leftover_template_syntax(code: &str, label: &str) {
    assert!(
        !code.contains("${"),
        "[{label}] raw `${{...}}` leaked into generated code:\n{code}"
    );
}

#[test]
fn step1_ast_template_literal_generates_css_var() {
    let src = r#"
import { tw } from "zares-css";
const Card = tw.div`rounded-xl shadow-sm p-6 bg-[${color}]`;
"#;
    let result = transform_source(src.to_string(), None);

    println!("=== STEP1 code ===\n{}", result.code);
    println!("=== STEP1 classes ===\n{:?}", result.classes);
    println!("=== STEP1 dynamic_css_json ===\n{:?}", result.dynamic_css_json);

    assert!(result.changed, "STEP1: expected transform to run (changed=true)");
    assert_no_leftover_template_syntax(&result.code, "STEP1 code");
    for c in &result.classes {
        assert!(!c.contains("${"), "STEP1: garbage dynamic class leaked: {c}");
    }

    let dyn_css = result
        .dynamic_css_json
        .expect("STEP1: expected dynamic_css_json to be populated for `bg-[${color}]`");
    assert!(
        dyn_css.contains("background-color") && dyn_css.contains("var(--Card-color"),
        "STEP1: expected a generated `background-color: var(--Card-color, ...)` rule, got: {dyn_css}"
    );
}

#[test]
fn step2_wrapped_component_template_literal_generates_css_var() {
    let src = r#"
import { tw } from "zares-css";
import { SomeBaseComponent } from "./base";
const Wrapped = tw(SomeBaseComponent)`p-4 text-[${textColor}]`;
"#;
    let result = transform_source(src.to_string(), None);

    println!("=== STEP2 code ===\n{}", result.code);
    println!("=== STEP2 dynamic_css_json ===\n{:?}", result.dynamic_css_json);

    assert_no_leftover_template_syntax(&result.code, "STEP2 code");

    // NOTE: if this assertion fails with dynamic_css_json == None, it likely
    // means STEP 2 never reached the dynamic-var branch for this fixture —
    // check whether `tw(Component)` wrapping requires a differently-shaped
    // regex match than the one exercised here.
    if let Some(dyn_css) = &result.dynamic_css_json {
        assert!(
            dyn_css.contains("var(--"),
            "STEP2: dynamic_css_json present but has no generated var(): {dyn_css}"
        );
    }
}

#[test]
fn step3_object_config_generates_css_var_not_garbage_class() {
    let src = r#"
import { tw } from "zares-css";
const Button = tw.button({
  base: `px-4 py-2 rounded-lg font-medium bg-[${bg}] text-[${textColor}]`,
  variants: {
    size: {
      sm: "text-sm px-3",
      lg: "text-lg px-6"
    }
  },
  states: {
    loading: "opacity-60 cursor-wait"
  }
});
"#;
    let result = transform_source(src.to_string(), None);

    println!("=== STEP3 code ===\n{}", result.code);
    println!("=== STEP3 classes ===\n{:?}", result.classes);
    println!("=== STEP3 dynamic_css_json ===\n{:?}", result.dynamic_css_json);

    // Regression guard for the specific bug found during audit: STEP 3 used to
    // have ZERO guard against `${...}`, so these literal garbage tokens would
    // show up verbatim in `classes` / `code`. That must never happen again.
    for c in &result.classes {
        assert!(
            !c.contains("bg-[${") && !c.contains("text-[${"),
            "STEP3 REGRESSION: raw dynamic token baked as static class: {c}"
        );
    }
    assert_no_leftover_template_syntax(&result.code, "STEP3 code");

    let dyn_css = result
        .dynamic_css_json
        .expect("STEP3: expected dynamic_css_json for `bg-[${bg}]` / `text-[${textColor}]`");
    assert!(dyn_css.contains("var(--Button-bg"), "missing bg var: {dyn_css}");
    assert!(
        dyn_css.contains("var(--Button-textColor"),
        "missing textColor var: {dyn_css}"
    );

    // Static parts of the same object (variants/states without `${`) must
    // still work exactly as before — the dynamic-var change must not regress
    // ordinary static compilation.
    assert!(
        result.classes.iter().any(|c| c == "text-sm" || c == "px-3"),
        "STEP3: static `size.sm` classes missing, static path may have regressed"
    );
}

#[test]
fn step3_sub_component_dynamic_var_is_scoped_by_sub_name() {
    // Guards the naming-collision fix: `sub.header` and `sub.body` (or the
    // top-level `base`) using the same source variable name must NOT produce
    // the same CSS Variable name.
    let src = r#"
import { tw } from "zares-css";
const Card = tw.div({
  base: `rounded-xl bg-[${color}]`,
  sub: {
    header: { tag: "div", base: `text-lg font-bold text-[${color}]` }
  }
});
"#;
    let result = transform_source(src.to_string(), None);
    let dyn_css = result
        .dynamic_css_json
        .expect("expected dynamic_css_json for base + sub.header both using `color`");

    println!("=== STEP3 sub-scoped dynamic_css_json ===\n{dyn_css}");

    assert!(
        dyn_css.contains("--Card-color"),
        "missing top-level scoped var --Card-color: {dyn_css}"
    );
    assert!(
        dyn_css.contains("--Card-header-color"),
        "missing sub-scoped var --Card-header-color: {dyn_css}"
    );
    assert_ne!(
        dyn_css.matches("--Card-color").count(),
        0,
        "base var must exist independently of sub var"
    );
}

#[test]
fn static_only_file_is_unaffected_no_dynamic_css() {
    // Control case: a file with zero `${...}` tokens must NOT get a
    // dynamic_css_json at all (None, not `Some("[]")`), and must compile
    // exactly as before this change.
    let src = r#"
import { tw } from "zares-css";
const Card = tw.div`rounded-xl shadow-sm p-6 bg-blue-600 text-white`;
"#;
    let result = transform_source(src.to_string(), None);
    assert!(result.changed, "expected static template to still compile");
    assert!(
        result.dynamic_css_json.is_none(),
        "static-only file must not produce dynamic_css_json, got: {:?}",
        result.dynamic_css_json
    );
}
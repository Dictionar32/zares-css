//! Repro tests — "Mode 2 Approach 3: Auto-generated props" (README §3.5)
//!
//! Covers the claim: `${x}` inside a `tw.object` definition should let the
//! consumer write `<Card bgColor={...} titleColor={...} />` directly — no
//! `style={{ "--Card-bgColor": ... }}`, no `setToken()`, no manual wiring.
//!
//! Since the generated output is a JS *string* (not something we can render
//! with real React in a Rust test), these tests assert on the SHAPE of the
//! generated `React.forwardRef` source: that it destructures the right prop
//! names, merges them into `style` alongside any caller-provided `style`,
//! and strips them off before they reach the underlying DOM element (so
//! React doesn't warn about unknown DOM attributes like `bgColor`).
//!
//! Run with:
//!   cd native && cargo test --test mode2_approach3_props_repro -- --nocapture

use tailwind_styled_parser::domain::transform::transform_source;

#[test]
fn step3_object_config_generates_destructured_props_not_style_only() {
    let src = r#"
import { tw } from "zares-css";
const Card = tw.div({
  base: `rounded-xl shadow-sm p-6 bg-[${bgColor}]`,
  sub: {
    header: { tag: "div", base: `text-lg font-bold text-[${titleColor}]` },
  },
});
"#;
    let result = transform_source(src.to_string(), None);
    println!("=== STEP3 Approach3 code ===\n{}", result.code);

    // The whole point of Approach 3: `props.bgColor` / `props.titleColor`
    // must appear in the generated source, driving the CSS vars directly.
    assert!(
        result.code.contains("props.bgColor"),
        "expected generated code to destructure `props.bgColor`, got:\n{}",
        result.code
    );
    assert!(
        result.code.contains("props.titleColor"),
        "expected generated code to destructure `props.titleColor`, got:\n{}",
        result.code
    );

    // Must merge with caller-provided `style`, not clobber it.
    assert!(
        result.code.contains("props.style"),
        "expected generated code to merge with `props.style`, got:\n{}",
        result.code
    );

    // Must not leak `bgColor`/`titleColor` onto the underlying DOM element —
    // i.e. they must be deleted from the props object passed to
    // React.createElement, or React will warn about unknown DOM attributes.
    assert!(
        result.code.contains("delete _p.bgColor") || result.code.contains("delete _r.bgColor"),
        "expected `bgColor` to be stripped before reaching the DOM element, got:\n{}",
        result.code
    );

    // The CSS var itself must still be generated (Mode 2 base functionality).
    let dyn_css = result.dynamic_css_json.expect("expected dynamic_css_json");
    assert!(dyn_css.contains("var(--Card-bgColor"), "{dyn_css}");
    assert!(dyn_css.contains("var(--Card-header-titleColor"), "{dyn_css}");
}

#[test]
fn step1_template_literal_generates_destructured_props() {
    let src = r#"
import { tw } from "zares-css";
const Card = tw.div`rounded-xl shadow-sm p-6 bg-[${bg}]`;
"#;
    let result = transform_source(src.to_string(), None);
    println!("=== STEP1 Approach3 code ===\n{}", result.code);

    assert!(
        result.code.contains("props.bg"),
        "expected `props.bg` destructured, got:\n{}",
        result.code
    );
    assert!(
        result.code.contains("style"),
        "expected a `style` merge in generated code, got:\n{}",
        result.code
    );
}

#[test]
fn static_only_component_unaffected_no_style_wiring_added() {
    // Control: a component with zero `${...}` tokens must NOT get any of the
    // Approach-3 destructure/style machinery — output must look exactly like
    // the plain STEP 1 static path from before this feature existed.
    let src = r#"
import { tw } from "zares-css";
const Card = tw.div`rounded-xl shadow-sm p-6 bg-blue-600 text-white`;
"#;
    let result = transform_source(src.to_string(), None);
    println!("=== static-only code ===\n{}", result.code);

    assert!(
        !result.code.contains("_ds") && !result.code.contains("_st"),
        "static-only component must not get dynamic-style scaffolding, got:\n{}",
        result.code
    );
    assert!(result.dynamic_css_json.is_none());
}

#[test]
fn reused_source_name_across_scopes_maps_to_single_prop() {
    // README §3.5 documented behaviour: if `${color}` is reused in both
    // `base` and `sub.header`, it becomes ONE prop (`color`) driving two
    // different CSS Variables — not two separate props.
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
    println!("=== reused-source code ===\n{}", result.code);

    // Only ONE destructure/delete of `color` should appear (dedup by source),
    // even though it feeds two separate CSS Variables.
    let delete_count = result.code.matches("delete _p.color;").count()
        + result.code.matches("delete _r.color;").count();
    assert_eq!(
        delete_count, 1,
        "expected exactly one `delete ...color;` (deduped prop), got {} in:\n{}",
        delete_count, result.code
    );

    let dyn_css = result.dynamic_css_json.expect("expected dynamic_css_json");
    assert!(dyn_css.contains("--Card-color"), "{dyn_css}");
    assert!(dyn_css.contains("--Card-header-color"), "{dyn_css}");
}

#[test]
fn arbitrary_property_syntax_disambiguates_text_prefix() {
    // Resolves limitation (a): `text-[${x}]` always means `color`. If you need
    // a dynamic `font-size` instead, use Tailwind's own arbitrary-property
    // syntax `[font-size:${x}]` — no prefix guessing, no ambiguity.
    let src = r#"
import { tw } from "zares-css";
const Label = tw.span`[font-size:${fontSize}] text-[${textColor}]`;
"#;
    let result = transform_source(src.to_string(), None);
    println!("=== arbitrary-property code ===\n{}", result.code);

    let dyn_css = result
        .dynamic_css_json
        .expect("expected dynamic_css_json for both dynamic tokens");
    println!("=== dynamic_css_json ===\n{dyn_css}");

    // `[font-size:${fontSize}]` must resolve to the LITERAL property named —
    // font-size — not get swallowed into the `text` prefix's `color` mapping.
    assert!(
        dyn_css.contains("font-size: var(--Label-fontSize"),
        "expected explicit `font-size` property from arbitrary-property syntax, got: {dyn_css}"
    );
    // `text-[${textColor}]` must still resolve to `color` as documented.
    assert!(
        dyn_css.contains("color: var(--Label-textColor"),
        "expected `text-[...]` to still resolve to `color`, got: {dyn_css}"
    );
    assert!(
        result.code.contains("props.fontSize") && result.code.contains("props.textColor"),
        "expected both props destructured (Approach 3), got:\n{}",
        result.code
    );
}

#[test]
fn official_tailwind_hint_syntax_disambiguates_text_prefix() {
    // This mirrors Tailwind's OWN documented "Resolving ambiguities" answer
    // (tailwindcss.com/docs/adding-custom-styles#resolving-ambiguities, v4.3):
    //   text-(length:--my-var) → font-size
    //   text-(color:--my-var)  → color
    // We adapt it for `${...}` dynamic tokens instead of a literal --my-var.
    let src = r#"
import { tw } from "zares-css";
const Label = tw.span`text-(length:${fontSize}) text-(color:${textColor})`;
"#;
    let result = transform_source(src.to_string(), None);
    println!("=== hint-parens code ===\n{}", result.code);

    let dyn_css = result
        .dynamic_css_json
        .expect("expected dynamic_css_json for both hinted dynamic tokens");
    println!("=== dynamic_css_json ===\n{dyn_css}");

    assert!(
        dyn_css.contains("font-size: var(--Label-fontSize"),
        "expected `text-(length:...)` to resolve to font-size, got: {dyn_css}"
    );
    assert!(
        dyn_css.contains("color: var(--Label-textColor"),
        "expected `text-(color:...)` to resolve to color, got: {dyn_css}"
    );
    assert!(
        result.code.contains("props.fontSize") && result.code.contains("props.textColor"),
        "expected both props destructured (Approach 3), got:\n{}",
        result.code
    );
}

#[test]
fn v3_bracket_hint_syntax_still_supported() {
    // Back-compat: the older Tailwind v3 bracket form of the same hint,
    // `text-[length:${x}]`, should resolve identically to the v4 parens form.
    let src = r#"
import { tw } from "zares-css";
const Label = tw.span`text-[length:${fontSize}]`;
"#;
    let result = transform_source(src.to_string(), None);
    let dyn_css = result.dynamic_css_json.expect("expected dynamic_css_json");
    assert!(
        dyn_css.contains("font-size: var(--Label-fontSize"),
        "expected v3 bracket hint form to resolve to font-size, got: {dyn_css}"
    );
}

#[test]
fn expanded_prefix_table_covers_hyphenated_multiword_prefixes() {
    // Limitation (b) mitigation: the prefix table now covers many more
    // Tailwind v4 utility categories, including multi-word/hyphenated
    // prefixes (border-t, translate-x, min-w, etc), confirmed against
    // official Tailwind docs (color-utility categories + spacing/sizing/
    // position/transform families).
    let src = r#"
import { tw } from "zares-css";
const Box = tw.div`border-t-[${topColor}] translate-x-[${offsetX}] min-w-[${minWidth}] outline-[${outlineColor}] ring-[${ringColor}]`;
"#;
    let result = transform_source(src.to_string(), None);
    println!("=== expanded-prefix code ===\n{}", result.code);
    let dyn_css = result.dynamic_css_json.expect("expected dynamic_css_json");
    println!("=== dynamic_css_json ===\n{dyn_css}");

    assert!(dyn_css.contains("border-top-color: var(--Box-topColor"), "{dyn_css}");
    assert!(dyn_css.contains("--tw-translate-x: var(--Box-offsetX"), "{dyn_css}");
    assert!(dyn_css.contains("min-width: var(--Box-minWidth"), "{dyn_css}");
    assert!(dyn_css.contains("outline-color: var(--Box-outlineColor"), "{dyn_css}");
    assert!(dyn_css.contains("--tw-ring-color: var(--Box-ringColor"), "{dyn_css}");
}
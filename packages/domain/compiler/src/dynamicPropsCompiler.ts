/**
 * dynamicPropsCompiler.ts
 *
 * Consumes `DynamicPropUsage[]` from `oxcExtractClasses()` (scanner package)
 * and decides, per prop usage, whether it can become a build-time static
 * class or whether it must fall back to a minimal runtime CSS write.
 *
 * ── Honesty note (read before wiring this in) ──────────────────────────────
 * `PropValueKind.ThemeResolvable` currently only carries the *root* import
 * identifier the expression was traced to (e.g. "theme" for
 * `bgColor={theme.primary}`) — the Rust visitor does not yet capture the
 * rest of the member-expression path ("primary"). Without that path, this
 * module cannot actually look up a value in your theme object, so
 * "theme_resolvable" is treated the same as "runtime" below: safe and
 * correct, just not build-time yet.
 *
 * To close this gap: extend `DynamicPropUsage` in
 * `native/src/oxc_parser.rs` with a `theme_path: string[]` field (populated
 * by walking the MemberExpression chain in `classify_expression`, collecting
 * each `.property` name), thread it through `oxc_api.rs` and
 * `oxc-bridge.ts`, then implement `resolveThemePath(theme, path)` here and
 * route successfully-resolved lookups into `staticClasses` instead of
 * `runtimeWrites`.
 * ────────────────────────────────────────────────────────────────────────
 *
 * What *is* fully build-time today: `kind === "static"` — literal values
 * (`bgColor="#1e293b"`) require no runtime mechanism at all.
 *
 * What is *minimal* runtime: `kind === "runtime"` (and, until the gap above
 * is closed, `"theme_resolvable"` too) — these get compiled down to a
 * single `element.style.setProperty(cssVarName, value)` call per prop,
 * targeting a CSS custom property that a build-time-generated class already
 * references. This is deliberately NOT `sheet.insertRule(...)` per distinct
 * value (see the "Class Name Generator + Constructable Stylesheets"
 * approach discussed earlier) — that approach lets the rule cache grow
 * unbounded for continuously-changing values (color pickers, drag sliders,
 * etc). `setProperty` is O(1) per write and never grows the stylesheet.
 */

import type { DynamicPropUsage } from "@tailwind-styled/scanner"

// CSS property names that dynamic prop attr names most commonly map to.
// This is intentionally small and explicit rather than a magic
// camelCase→kebab-case transform, so unrecognized attr names fail loudly
// instead of silently generating a nonsense CSS property.
const KNOWN_CSS_PROPERTIES: Record<string, string> = {
  bgColor: "background-color",
  textColor: "color",
  borderColor: "border-color",
  color: "color",
  width: "width",
  height: "height",
  padding: "padding",
  margin: "margin",
  fontSize: "font-size",
  gap: "gap",
}

export interface StaticClassEntry {
  componentName: string
  attrName: string
  className: string
  cssProperty: string
  /** Only present for `kind === "static"` entries — literal value baked into the rule. */
  literalValue: string
}

export interface RuntimeWriteEntry {
  componentName: string
  attrName: string
  cssProperty: string
  /** CSS custom property name the build-time class references, e.g. "--zares-bgColor". */
  cssVarName: string
  /**
   * Why this couldn't be resolved at build time — surfaced so tooling/logs
   * can explain *why* a given prop is runtime instead of silently treating
   * every non-static prop the same way.
   */
  reason: "runtime-value" | "theme-path-not-captured-yet"
}

export interface DynamicPropsCompileResult {
  staticClasses: StaticClassEntry[]
  runtimeWrites: RuntimeWriteEntry[]
  /** attrName values that had no entry in KNOWN_CSS_PROPERTIES — nothing was generated for these. */
  unknownAttrs: string[]
}

function cssVarNameFor(attrName: string): string {
  return `--zares-${attrName}`
}

function classNameFor(componentName: string, attrName: string): string {
  return `zares-${componentName}-${attrName}`
}

/**
 * Splits classified dynamic prop usages into build-time-static class
 * definitions and minimal-runtime write instructions.
 *
 * `literalValues` maps `${componentName}.${attrName}` → the literal string
 * value the scanner saw for `kind === "static"` entries. The Rust visitor
 * currently classifies *that* a value is static but does not extract the
 * literal's text — if you need `literalValue` populated, either extend
 * `classify_expression`/`DynamicPropUsage` in Rust to carry it, or pass it
 * in here from a source you already have (e.g. a second, simpler regex/AST
 * pass over the same file). Until then, static entries without a matching
 * key in `literalValues` are skipped with a console warning rather than
 * emitting a class with a missing/fake value.
 */
export function compileDynamicProps(
  usages: DynamicPropUsage[],
  literalValues: Record<string, string> = {}
): DynamicPropsCompileResult {
  const staticClasses: StaticClassEntry[] = []
  const runtimeWrites: RuntimeWriteEntry[] = []
  const unknownAttrs = new Set<string>()

  for (const usage of usages) {
    const cssProperty = KNOWN_CSS_PROPERTIES[usage.attrName]
    if (!cssProperty) {
      unknownAttrs.add(usage.attrName)
      continue
    }

    if (usage.kind === "static") {
      const key = `${usage.componentName}.${usage.attrName}`
      const literalValue = literalValues[key]
      if (literalValue === undefined) {
        console.warn(
          `[zares-css] dynamic prop "${usage.attrName}" on "${usage.componentName}" ` +
          `was classified as static but no literal value was provided — skipping. ` +
          `See dynamicPropsCompiler.ts doc comment for how to wire literalValues.`
        )
        continue
      }
      staticClasses.push({
        componentName: usage.componentName,
        attrName: usage.attrName,
        className: classNameFor(usage.componentName, usage.attrName),
        cssProperty,
        literalValue,
      })
      continue
    }

    // "runtime" and (for now, see doc comment) "theme_resolvable" both go
    // through the minimal-runtime path.
    runtimeWrites.push({
      componentName: usage.componentName,
      attrName: usage.attrName,
      cssProperty,
      cssVarName: cssVarNameFor(usage.attrName),
      reason: usage.kind === "theme_resolvable" ? "theme-path-not-captured-yet" : "runtime-value",
    })
  }

  return {
    staticClasses,
    runtimeWrites,
    unknownAttrs: [...unknownAttrs],
  }
}

/**
 * Generates the build-time CSS for `staticClasses` plus the *structural*
 * rules `runtimeWrites` need (a class whose property reads from a CSS
 * custom property with a safe fallback) — the structural rule itself is
 * static and cacheable; only the custom property's value is written at
 * runtime.
 */
export function generateDynamicPropsCss(result: DynamicPropsCompileResult): string {
  const lines: string[] = []

  for (const entry of result.staticClasses) {
    lines.push(`.${entry.className} { ${entry.cssProperty}: ${entry.literalValue}; }`)
  }

  // De-dupe structural rules per cssVarName — multiple components can share
  // the same attr name (e.g. every "bgColor" usage reuses `--zares-bgColor`).
  const seenVars = new Set<string>()
  for (const entry of result.runtimeWrites) {
    if (seenVars.has(entry.cssVarName)) continue
    seenVars.add(entry.cssVarName)
    lines.push(
      `.zares-dynamic-${entry.attrName} { ${entry.cssProperty}: var(${entry.cssVarName}, initial); }`
    )
  }

  return lines.join("\n")
}
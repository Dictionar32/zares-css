/**
 * dynamicPropWrite.ts
 *
 * The ONE runtime mechanism this package uses for dynamic prop values that
 * can't be resolved at build time (see dynamicPropsCompiler.ts in the
 * compiler package for how a prop usage ends up here).
 *
 * Deliberately just `element.style.setProperty(...)` — no `insertRule`, no
 * generated class-per-value, no Constructable Stylesheet. Those approaches
 * were considered and rejected earlier: they either don't work under SSR
 * without a separate fallback, or grow a class/rule cache unboundedly for
 * continuously-changing values (drag sliders, color pickers). `setProperty`
 * is O(1) per write, keeps the stylesheet static, and needs no hydration
 * dance — it's the "there is always a tiny bit of runtime, minimize it"
 * conclusion from the zero-runtime discussion.
 *
 * Pairs with the structural CSS `generateDynamicPropsCss()` emits at build
 * time, e.g.:
 *   .zares-dynamic-bgColor { background-color: var(--zares-bgColor, initial); }
 * This function only ever writes the custom property's value — the rule
 * that consumes it is static and already shipped in the build.
 */

const cssVarNameFor = (attrName: string): string => `--zares-${attrName}`

/**
 * Writes a single dynamic prop's value onto an element as a CSS custom
 * property. No-ops on the server (SSR) — there is no DOM to write to, and
 * the structural class's `var(..., initial)` fallback keeps the element
 * rendering something sane until this runs client-side post-hydration.
 */
export function applyDynamicProp(
  element: HTMLElement | null | undefined,
  attrName: string,
  value: string | number | undefined
): void {
  if (typeof window === "undefined" || !element) return
  if (value === undefined) {
    element.style.removeProperty(cssVarNameFor(attrName))
    return
  }
  element.style.setProperty(cssVarNameFor(attrName), String(value))
}

/**
 * Batch form of `applyDynamicProp` for components with multiple dynamic
 * props (e.g. `bgColor` + `textColor` on the same element) — avoids forcing
 * a style recalculation per property when several change together.
 */
export function applyDynamicProps(
  element: HTMLElement | null | undefined,
  props: Record<string, string | number | undefined>
): void {
  if (typeof window === "undefined" || !element) return
  for (const [attrName, value] of Object.entries(props)) {
    applyDynamicProp(element, attrName, value)
  }
}
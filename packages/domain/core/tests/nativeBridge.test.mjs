/**
 * Core Native Bindings — extractThemeFromCss
 * 
 * Tests for the extractThemeFromCss function wired from Rust.
 */

import { test, describe } from "node:test"
import assert from "node:assert/strict"

describe("Core Native Bindings — extractThemeFromCss", () => {
  test("should export extractThemeFromCss", async () => {
    const { extractThemeFromCss } = await import("@tailwind-styled/core")
    assert.equal(typeof extractThemeFromCss, "function")
  })

  test("should be callable without throwing when native is unavailable", async () => {
    const { extractThemeFromCss } = await import("@tailwind-styled/core")
    assert.doesNotThrow(() => {
      try {
        extractThemeFromCss("@theme { --color-primary: #3b82f6; }")
      } catch (err) {
        if (String(err).includes("not available")) {
          return
        }
        throw err
      }
    })
  })
})

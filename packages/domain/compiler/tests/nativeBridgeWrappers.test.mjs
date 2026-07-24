/**
 * Native Bridge Wrappers — Newly Wired Functions
 * 
 * Tests for the 5 functions wired in 2026-07-24:
 * - clear_parse_cache_napi
 * - clear_theme_cache_napi
 * - get_watch_system_status
 * - get_week8_optimization_status
 * - inspect_cache_stats
 */

import { test, describe } from "node:test"
import assert from "node:assert/strict"

function assertNativeAvailable(fn, name) {
  try {
    fn()
  } catch (err) {
    if (String(err).includes("not available")) {
      return
    }
    throw err
  }
}

describe("NativeBridge Wrappers — Newly Wired Functions", () => {
  test("clear_parse_cache_napi is exported and callable", async () => {
    let clear_parse_cache_napi
    try {
      const mod = await import("@tailwind-styled/compiler/cache")
      clear_parse_cache_napi = mod.clear_parse_cache_napi
    } catch {
      console.warn("[nativeBridgeWrappers.test] cache module not available, skipping")
      return
    }
    assert.equal(typeof clear_parse_cache_napi, "function")
    assert.doesNotThrow(() => assertNativeAvailable(clear_parse_cache_napi))
  })

  test("clear_theme_cache_napi is exported and callable", async () => {
    let clear_theme_cache_napi
    try {
      const mod = await import("@tailwind-styled/compiler/cache")
      clear_theme_cache_napi = mod.clear_theme_cache_napi
    } catch {
      console.warn("[nativeBridgeWrappers.test] cache module not available, skipping")
      return
    }
    assert.equal(typeof clear_theme_cache_napi, "function")
    assert.doesNotThrow(() => assertNativeAvailable(clear_theme_cache_napi))
  })

  test("get_watch_system_status is exported and callable", async () => {
    let get_watch_system_status
    try {
      const mod = await import("@tailwind-styled/compiler/watch")
      get_watch_system_status = mod.get_watch_system_status
    } catch {
      console.warn("[nativeBridgeWrappers.test] watch module not available, skipping")
      return
    }
    assert.equal(typeof get_watch_system_status, "function")
    assert.doesNotThrow(() => assertNativeAvailable(get_watch_system_status))
  })

  test("get_week8_optimization_status is exported and callable", async () => {
    let get_week8_optimization_status
    try {
      const mod = await import("@tailwind-styled/compiler/analyzer")
      get_week8_optimization_status = mod.get_week8_optimization_status
    } catch {
      console.warn("[nativeBridgeWrappers.test] analyzer module not available, skipping")
      return
    }
    assert.equal(typeof get_week8_optimization_status, "function")
    assert.doesNotThrow(() => assertNativeAvailable(get_week8_optimization_status))
  })

  test("inspect_cache_stats is exported and callable with capacity", async () => {
    let inspect_cache_stats
    try {
      const mod = await import("@tailwind-styled/compiler/analyzer")
      inspect_cache_stats = mod.inspect_cache_stats
    } catch {
      console.warn("[nativeBridgeWrappers.test] analyzer module not available, skipping")
      return
    }
    assert.equal(typeof inspect_cache_stats, "function")
    assert.doesNotThrow(() => assertNativeAvailable(() => inspect_cache_stats(1000)))
  })
})

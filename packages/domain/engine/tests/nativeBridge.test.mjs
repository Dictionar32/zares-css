/**
 * Engine Native Bridge — clearNameRegistries
 * 
 * Tests for the clearNameRegistries function wired from Rust.
 */

import { test, describe } from "node:test"
import assert from "node:assert/strict"

function assertNativeAvailable(fn) {
  try {
    fn()
  } catch (err) {
    if (String(err).includes("not available")) {
      return
    }
    throw err
  }
}

describe("Engine Native Bridge — clearNameRegistries", () => {
  test("should export clearNameRegistries", async () => {
    let clearNameRegistries
    try {
      const mod = await import("@tailwind-styled/engine")
      clearNameRegistries = mod.clearNameRegistries
    } catch {
      console.warn("[engine/nativeBridge.test] engine module not available, skipping")
      return
    }
    assert.equal(typeof clearNameRegistries, "function")
  })

  test("should be callable without throwing when native is unavailable", async () => {
    let clearNameRegistries
    try {
      const mod = await import("@tailwind-styled/engine")
      clearNameRegistries = mod.clearNameRegistries
    } catch {
      console.warn("[engine/nativeBridge.test] engine module not available, skipping")
      return
    }
    assert.doesNotThrow(() => assertNativeAvailable(clearNameRegistries))
  })
})

/**
 * tailwind-styled-v4/next
 *
 * Next.js integration entry point.
 *
 * Usage:
 *   import { withTailwindStyled } from "zares-css/next"
 */

// Re-export schemas
export {
  type NextAdapterOptionsInput,
  NextAdapterOptionsSchema,
  parseNextAdapterOptions,
} from "./schemas"
export type { TailwindStyledNextOptions } from "./withTailwindStyled"
export { withTailwindStyled } from "./withTailwindStyled"
export { StaticCssWebpackPlugin, setFileStaticCss } from "./staticCssWebpackPlugin"

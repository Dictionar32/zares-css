import { withTailwindStyled } from "zares-css/next"
import type { NextConfig } from 'next'

const nextConfig: NextConfig = {}

export default withTailwindStyled({ autoClientBoundary: false })(nextConfig)

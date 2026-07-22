import type { Metadata } from "next";
import Script from "next/script";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import { TwCssInjector } from "zares-css/runtime-css";
import { ThemeProvider } from "@/components/ThemeProvider";
import { tw } from "zares-css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "tailwind-styled v4.5 demo",
  description: "Next.js App Router demo with tailwind-styled-v4.",
};

/**
 * RootBody — styled with tw.*
 * Pakai template literal untuk dynamic CSS variables dari geist fonts
 */
const RootBody = tw.body({
  base: "antialiased",
  style: {
    fontFamily: `var(--font-geist-sans), var(--font-geist-mono), system-ui, sans-serif`,
  },
});

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" style={{ "--font-geist-sans": geistSans.variable, "--font-geist-mono": geistMono.variable } as React.CSSProperties}>
      <head>
        {/*
         * TwCssInjector — inject route-specific CSS inline ke <head>.
         * Server Component async — harus import dari runtime-css (server-safe entry).
         */}
        <TwCssInjector />
        <Script
          src="https://context7.com/widget.js"
          data-library="/dictionar32/zares-css"
          strategy="afterInteractive"
        />
      </head>
      <RootBody>
        <ThemeProvider>
          {children}
        </ThemeProvider>
      </RootBody>
    </html>
  );
}

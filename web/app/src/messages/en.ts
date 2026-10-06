// SPDX-License-Identifier: Apache-2.0
// The source catalog. Every other language has exactly these keys.
// Messages use ICU MessageFormat.

export const en = {
  "shell.skip": "Skip to content",
  "about.title": "About Scoplen",
  "about.summary": "An SSH client and bastion, fully self-hosted.",
  "about.version": "Version {version} for {platform, select, macos {macOS} windows {Windows} other {this system}}",
  "about.maturity": "Maturity: Exploration. This build is the application shell; it does not connect to hosts yet.",
  "about.project": "Scoplen is a Plystra project.",
  "about.license": "The client is open source under the Apache License 2.0.",
  "startup.error.title": "Scoplen could not start its interface",
  "startup.error.body":
    "The window could not reach the application core. Nothing was changed. Try again; if it fails again, quit and reopen Scoplen.",
  "startup.error.reference": "Reference: {reference}",
  "startup.error.retry": "Try again",
} as const;

export type Messages = { [K in keyof typeof en]: string };

// SPDX-License-Identifier: Apache-2.0
// The source catalog. Every other language has exactly these keys.
// Messages use ICU MessageFormat.

export const en = {
  "shell.skip": "Skip to content",
  "shell.nav": "Main",
  "shell.nav.about": "About",
  "shell.nav.settings": "Settings",

  "about.title": "About Scoplen",
  "about.summary": "An SSH client and bastion, fully self-hosted.",
  "about.version": "Version {version} for {platform, select, macos {macOS} windows {Windows} other {this system}}",
  "about.maturity":
    "Maturity: Exploration. This build stores its data encrypted on this device; it does not connect to hosts yet.",
  "about.project": "Scoplen is a Plystra project.",
  "about.license": "The client is open source under the Apache License 2.0.",

  "startup.error.title": "Scoplen could not start its interface",
  "startup.error.body":
    "The window could not reach the application core. Nothing was changed. Try again; if it fails again, quit and reopen Scoplen.",
  "startup.error.reference": "Reference: {reference}",
  "startup.error.retry": "Try again",

  "failed.body":
    "Something went wrong. Nothing was changed. Try again; if it keeps failing, report it with this reference.",

  "unlock.title": "Enter your local passphrase",
  "unlock.body": "The hosts and keys on this device are encrypted with it.",
  "unlock.field": "Local passphrase",
  "unlock.submit": "Unlock",
  "unlock.wrong": "That passphrase is not correct. Nothing was changed; try again.",

  "create.title": "Choose a local passphrase",
  "create.body":
    "This system has no keystore to protect Scoplen's data, so it is encrypted with a passphrase you enter each time Scoplen starts.",
  "create.field": "Passphrase",
  "create.confirm": "Repeat passphrase",
  "create.warning": "If you forget it, the data on this device cannot be recovered.",
  "create.submit": "Continue",

  "passphrase.empty": "Enter a passphrase.",
  "passphrase.mismatch": "The two passphrases are different.",

  "unreadable.title": "Scoplen cannot open its local data",
  "unreadable.keyMissing":
    "The key that protects the local data on this device is missing from the system keystore, so the data cannot be read.",
  "unreadable.wrongKey":
    "The key in the system keystore does not match the local data on this device, so the data cannot be read.",
  "unreadable.newerVersion":
    "The local data on this device was last used by a newer version of Scoplen. Install that version or a later one to open it.",
  "unreadable.startFresh":
    "You can start with empty local data. The unreadable data stays on this device under a new name; it is not deleted.",
  "unreadable.start": "Start with empty data",
  "unreadable.confirm":
    "Start with empty data? Hosts and keys that exist only in the unreadable data will not be available in Scoplen.",
  "unreadable.cancel": "Cancel",

  "settings.title": "Settings",
  "settings.passphrase.title": "Local passphrase",
  "settings.passphrase.keystore":
    "Your local data is protected by the system keystore. Set a passphrase to also require it each time Scoplen starts.",
  "settings.passphrase.keystoreAndPassphrase":
    "Scoplen asks for your passphrase each time it starts, in addition to the system keystore.",
  "settings.passphrase.passphraseOnly":
    "This system has no keystore, so your local data is protected by your passphrase alone.",
  "settings.passphrase.set": "Set passphrase",
  "settings.passphrase.change": "Change passphrase",
  "settings.passphrase.remove": "Remove passphrase",
  "settings.passphrase.new": "New passphrase",
  "settings.passphrase.repeat": "Repeat new passphrase",
  "settings.passphrase.warning": "If you forget it, the local data cannot be recovered.",
  "settings.passphrase.save": "Save passphrase",
  "settings.passphrase.cancel": "Cancel",
  "settings.passphrase.saved": "Passphrase saved.",
  "settings.passphrase.removed": "Passphrase removed.",
} as const;

export type Messages = { [K in keyof typeof en]: string };

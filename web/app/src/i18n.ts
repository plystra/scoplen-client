// SPDX-License-Identifier: Apache-2.0
import { createI18n } from "@scoplen/ui";
import type { Locale } from "./ipc/bindings";
import { en } from "./messages/en";
import { zhHans } from "./messages/zh-Hans";

export const catalogs = { en, "zh-Hans": zhHans } satisfies Record<Locale, Record<keyof typeof en, string>>;

export const { I18nProvider, useI18n } = createI18n<typeof en, Locale>(catalogs);

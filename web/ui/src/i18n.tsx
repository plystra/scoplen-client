// SPDX-License-Identifier: Apache-2.0
import { IntlMessageFormat, type PrimitiveType } from "intl-messageformat";
import { createContext, useContext, useMemo, type ReactNode } from "react";

/** Messages in ICU MessageFormat, keyed by a stable identifier. */
export type Catalog = Record<string, string>;

/** Values substituted into a message's placeholders. */
export type MessageValues = Record<string, PrimitiveType>;

export interface Translator<K extends string> {
  /** The BCP 47 tag of the language. */
  locale: string;
  /** Formats a message; plurals, selects, numbers, and dates follow the language's rules. */
  t: (key: K, values?: MessageValues) => string;
}

/**
 * Builds a translator over one language's catalog. Each message is parsed
 * once, on first use. A message whose ICU syntax is invalid throws when it is
 * first formatted; the catalog tests format every message to catch that.
 */
export function createTranslator<C extends Catalog>(locale: string, catalog: C): Translator<keyof C & string> {
  const cache = new Map<string, IntlMessageFormat>();
  return {
    locale,
    t(key, values) {
      let format = cache.get(key);
      if (!format) {
        const source = catalog[key];
        if (source === undefined) throw new Error(`No message "${key}" in the ${locale} catalog`);
        format = new IntlMessageFormat(source, locale, undefined, { ignoreTag: true });
        cache.set(key, format);
      }
      return String(format.format(values));
    },
  };
}

/**
 * Creates the provider and hook for an application's catalogs. `S` is the
 * source (English) catalog; every other language must have the same keys,
 * which the type of `catalogs` enforces.
 */
export function createI18n<S extends Catalog, L extends string>(catalogs: Record<L, { [K in keyof S]: string }>) {
  const Context = createContext<Translator<keyof S & string> | null>(null);

  function I18nProvider({ locale, children }: { locale: L; children: ReactNode }) {
    const translator = useMemo(() => createTranslator(locale, catalogs[locale]), [locale]);
    return <Context.Provider value={translator}>{children}</Context.Provider>;
  }

  function useI18n(): Translator<keyof S & string> {
    const translator = useContext(Context);
    if (!translator) throw new Error("useI18n must be used inside I18nProvider");
    return translator;
  }

  return { I18nProvider, useI18n };
}

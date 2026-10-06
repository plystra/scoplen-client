// SPDX-License-Identifier: Apache-2.0
import { createTranslator } from "../src";

const en = {
  hosts: "{count, plural, =0 {No hosts} one {# host} other {# hosts}}",
  greeting: "Connected to {host}",
};
const zh = {
  hosts: "{count, plural, =0 {没有主机} other {# 台主机}}",
  greeting: "已连接到 {host}",
};

describe("createTranslator", () => {
  it("applies each language's plural rules", () => {
    const t = createTranslator("en", en).t;
    expect(t("hosts", { count: 0 })).toBe("No hosts");
    expect(t("hosts", { count: 1 })).toBe("1 host");
    expect(t("hosts", { count: 1200 })).toBe("1,200 hosts");
    const z = createTranslator("zh-Hans", zh).t;
    expect(z("hosts", { count: 1 })).toBe("1 台主机");
    expect(z("hosts", { count: 0 })).toBe("没有主机");
  });

  it("substitutes values", () => {
    expect(createTranslator("zh-Hans", zh).t("greeting", { host: "prod-db-01" })).toBe("已连接到 prod-db-01");
  });

  it("rejects an unknown key", () => {
    const t = createTranslator("en", en).t as (key: string) => string;
    expect(() => t("missing")).toThrow('No message "missing" in the en catalog');
  });

  it("rejects a message with invalid ICU syntax", () => {
    const t = createTranslator("en", { broken: "{count, plural, one {x}" }).t;
    expect(() => t("broken", { count: 1 })).toThrow();
  });
});

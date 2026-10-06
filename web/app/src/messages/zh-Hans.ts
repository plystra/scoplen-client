// SPDX-License-Identifier: Apache-2.0
// 简体中文。从英文源文本出发，按中文的习惯来写，而不是逐句翻译。
import type { Messages } from "./en";

export const zhHans: Messages = {
  "shell.skip": "跳到主要内容",
  "about.title": "关于 Scoplen",
  "about.summary": "SSH 客户端和堡垒机，完全自己部署。",
  "about.version": "版本 {version}，适用于 {platform, select, macos {macOS} windows {Windows} other {当前系统}}",
  "about.maturity": "成熟度：探索阶段。这个版本只有应用框架，还不能连接主机。",
  "about.project": "Scoplen 是 Plystra 旗下的项目。",
  "about.license": "客户端以 Apache License 2.0 开源。",
  "startup.error.title": "Scoplen 界面没能启动",
  "startup.error.body": "窗口连不上应用核心，没有改动任何数据。请再试一次；如果还是失败，退出后重新打开 Scoplen。",
  "startup.error.reference": "参考信息：{reference}",
  "startup.error.retry": "再试一次",
};

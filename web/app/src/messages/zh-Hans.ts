// SPDX-License-Identifier: Apache-2.0
// 简体中文。从英文源文本出发，按中文的习惯来写，而不是逐句翻译。
import type { Messages } from "./en";

export const zhHans: Messages = {
  "shell.skip": "跳到主要内容",
  "shell.nav": "主导航",
  "shell.nav.about": "关于",
  "shell.nav.settings": "设置",

  "about.title": "关于 Scoplen",
  "about.summary": "SSH 客户端和堡垒机，完全自己部署。",
  "about.version": "版本 {version}，适用于 {platform, select, macos {macOS} windows {Windows} other {当前系统}}",
  "about.maturity": "成熟度：探索阶段。这个版本会把数据加密保存在本机，还不能连接主机。",
  "about.project": "Scoplen 是 Plystra 旗下的项目。",
  "about.license": "客户端以 Apache License 2.0 开源。",

  "startup.error.title": "Scoplen 界面没能启动",
  "startup.error.body": "窗口连不上应用核心，没有改动任何数据。请再试一次；如果还是失败，退出后重新打开 Scoplen。",
  "startup.error.reference": "参考信息：{reference}",
  "startup.error.retry": "再试一次",

  "failed.body": "出了点问题，没有改动任何数据。请再试一次；如果一直失败，请附上这条参考信息反馈给我们。",

  "unlock.title": "输入本地口令",
  "unlock.body": "这台设备上的主机和密钥都用它加密。",
  "unlock.field": "本地口令",
  "unlock.submit": "解锁",
  "unlock.wrong": "口令不对，没有改动任何数据，请再试一次。",

  "create.title": "设置本地口令",
  "create.body": "这个系统没有可用的密钥库，Scoplen 的数据只能用口令加密，每次启动时都要输入。",
  "create.field": "口令",
  "create.confirm": "再输一遍",
  "create.warning": "忘了口令，这台设备上的数据就找不回来了。",
  "create.submit": "继续",

  "passphrase.empty": "请输入口令。",
  "passphrase.mismatch": "两次输入的口令不一样。",

  "unreadable.title": "Scoplen 打不开本地数据",
  "unreadable.keyMissing": "保护本地数据的密钥已经不在系统密钥库里了，所以读不出这些数据。",
  "unreadable.wrongKey": "系统密钥库里的密钥和这台设备上的本地数据对不上，所以读不出这些数据。",
  "unreadable.newerVersion": "这台设备上的本地数据被更新版本的 Scoplen 用过。请安装那个版本或更新的版本来打开。",
  "unreadable.startFresh": "你可以从空数据重新开始。读不出的数据会换个名字留在这台设备上，不会被删除。",
  "unreadable.start": "从空数据开始",
  "unreadable.confirm": "确定从空数据开始吗？只存在于这份数据里的主机和密钥，在 Scoplen 里将无法使用。",
  "unreadable.cancel": "取消",

  "settings.title": "设置",
  "settings.passphrase.title": "本地口令",
  "settings.passphrase.keystore": "本地数据由系统密钥库保护。设置口令后，每次启动 Scoplen 还要输入它。",
  "settings.passphrase.keystoreAndPassphrase": "除了系统密钥库，Scoplen 每次启动时还会要求输入你的口令。",
  "settings.passphrase.passphraseOnly": "这个系统没有密钥库，本地数据只靠你的口令保护。",
  "settings.passphrase.set": "设置口令",
  "settings.passphrase.change": "修改口令",
  "settings.passphrase.remove": "取消口令",
  "settings.passphrase.new": "新口令",
  "settings.passphrase.repeat": "再输一遍新口令",
  "settings.passphrase.warning": "忘了口令，本地数据就找不回来了。",
  "settings.passphrase.save": "保存口令",
  "settings.passphrase.cancel": "取消",
  "settings.passphrase.saved": "口令已保存。",
  "settings.passphrase.removed": "口令已取消。",
};

// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from "react";
import { useI18n } from "./i18n";
import type { AppInfo, Status } from "./ipc/bindings";
import { Frame } from "./frame";
import { HostsHome } from "./inventory/home";
import { InventoryContext } from "./inventory/api";
import { createInventoryApi } from "./inventory/core-api";
import { InventoryObjects, type ObjectSection } from "./inventory/objects";
import { PassphraseSettings } from "./local-data";

type View = "hosts" | "objects" | "settings";

/** The application once its local data is open. */
export function Shell({
  info,
  status,
  onStatus,
}: {
  info: AppInfo;
  status: Status & { state: "open" };
  onStatus: (status: Status) => void;
}) {
  const { t } = useI18n();
  const [view, setView] = useState<View>("hosts");
  const [objectSection, setObjectSection] = useState<ObjectSection>("logins");
  const inventory = useMemo(() => createInventoryApi(), []);

  const openObjects = (section: ObjectSection) => {
    setObjectSection(section);
    setView("objects");
  };

  return (
    <Frame
      platform={info.platform}
      tabs={[{ id: "hosts", label: t("frame.hosts") }]}
      active="hosts"
      onTab={() => setView("hosts")}
      onSettings={() => setView(view === "settings" ? "hosts" : "settings")}
      settingsActive={view === "settings"}
    >
      {view === "settings" ? (
        <main id="main" tabIndex={-1} className="mx-auto max-w-xl px-6 py-10 focus:outline-none">
          <Settings status={status} onStatus={onStatus} />
        </main>
      ) : view === "objects" ? (
        <InventoryContext.Provider value={inventory}>
          <InventoryObjects section={objectSection} onBack={() => setView("hosts")} />
        </InventoryContext.Provider>
      ) : (
        <InventoryContext.Provider value={inventory}>
          <main id="main" tabIndex={-1} className="h-full min-h-0 focus:outline-none">
            <HostsHome onOpenObjects={openObjects} />
          </main>
        </InventoryContext.Provider>
      )}
    </Frame>
  );
}

function Settings({ status, onStatus }: { status: Status & { state: "open" }; onStatus: (status: Status) => void }) {
  const { t } = useI18n();
  return (
    <>
      <h1 className="font-serif text-2xl font-medium">{t("settings.title")}</h1>
      <PassphraseSettings status={status} onStatus={onStatus} />
    </>
  );
}

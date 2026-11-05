import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { DashboardPopover } from "./components/DashboardPopover";
import { HoverCard } from "./components/HoverCard";
import { FullUsagePayload, ProviderId } from "./types/usage";
import "./styles.css";

function App() {
  const [windowLabel] = useState<string>(() => {
    try {
      return getCurrentWebviewWindow().label;
    } catch {
      return "main-popover";
    }
  });

  const [data, setData] = useState<FullUsagePayload | null>(null);
  const [hoverProvider, setHoverProvider] = useState<ProviderId>("claude");
  const [activeTab, setActiveTab] = useState<ProviderId | "settings">("claude");
  const [nowEpochSec, setNowEpochSec] = useState<number>(() =>
    Math.floor(Date.now() / 1000)
  );

  useEffect(() => {
    invoke<FullUsagePayload>("get_usage_data")
      .then(setData)
      .catch(console.error);

    const unlistenUsage = listen<FullUsagePayload>("usage-updated", (ev) => {
      setData(ev.payload);
    });

    const unlistenHover = listen<string>("hover-provider", (ev) => {
      if (ev.payload === "claude" || ev.payload === "agy") {
        setHoverProvider(ev.payload);
      }
    });

    const unlistenSelect = listen<string>("select-provider", (ev) => {
      if (ev.payload === "claude" || ev.payload === "agy") {
        setActiveTab(ev.payload);
      }
    });

    const timer = window.setInterval(() => {
      setNowEpochSec(Math.floor(Date.now() / 1000));
    }, 15_000);

    return () => {
      unlistenUsage.then((fn) => fn());
      unlistenHover.then((fn) => fn());
      unlistenSelect.then((fn) => fn());
      window.clearInterval(timer);
    };
  }, []);

  if (windowLabel === "hover-card") {
    return (
      <HoverCard
        data={data}
        provider={hoverProvider}
        nowEpochSec={nowEpochSec}
      />
    );
  }

  return (
    <DashboardPopover
      data={data}
      activeTab={activeTab}
      onSelectTab={setActiveTab}
      onDataUpdated={setData}
      nowEpochSec={nowEpochSec}
    />
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

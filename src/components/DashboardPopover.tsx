import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Settings } from "lucide-react";
import {
  FullUsagePayload,
  ProviderId,
  RangeKey,
} from "../types/usage";
import { BrandIcon } from "./BrandIcon";
import { ModelBreakdown } from "./ModelBreakdown";
import { QuotaBar } from "./QuotaBar";
import { SettingsPanel } from "./SettingsPanel";
import { TokenGrid } from "./TokenGrid";
import { UsageChart } from "./UsageChart";

interface DashboardPopoverProps {
  data: FullUsagePayload | null;
  activeTab: ProviderId | "settings";
  onSelectTab: (tab: ProviderId | "settings") => void;
  onDataUpdated: (payload: FullUsagePayload) => void;
  nowEpochSec: number;
}

const RANGE_OPTIONS: { key: RangeKey; label: string }[] = [
  { key: "five_hour", label: "รอบ 5h" },
  { key: "today", label: "วันนี้" },
  { key: "seven_day", label: "7 วัน" },
  { key: "thirty_day", label: "30 วัน" },
];

export function DashboardPopover({
  data,
  activeTab,
  onSelectTab,
  onDataUpdated,
  nowEpochSec,
}: DashboardPopoverProps) {
  const [rangeKey, setRangeKey] = useState<RangeKey>("five_hour");
  const [refreshing, setRefreshing] = useState(false);

  const handleManualRefresh = async () => {
    setRefreshing(true);
    try {
      const fresh = await invoke<FullUsagePayload>("force_refresh");
      onDataUpdated(fresh);
    } finally {
      setTimeout(() => setRefreshing(false), 250);
    }
  };

  if (!data) {
    return (
      <div className="popover-shell w-full h-full p-4 flex items-center justify-center text-xs text-[var(--text-muted)]">
        กำลังสแกนข้อมูล Local Usage...
      </div>
    );
  }

  const activeProvider: ProviderId =
    activeTab === "agy" ? "agy" : "claude";
  const report = activeProvider === "claude" ? data.claude : data.agy;
  const currentRangeStats = report.ranges[rangeKey];

  return (
    <div className="popover-shell w-full h-full flex flex-col overflow-hidden">
      {/* Top Navigation Bar */}
      <div className="px-3 pt-3 pb-2 border-b border-[var(--card-border)] flex items-center justify-between gap-2 shrink-0">
        <div className="flex items-center gap-1 p-0.5 rounded-lg bg-[var(--meter-track)]">
          <button
            type="button"
            onClick={() => onSelectTab("claude")}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-semibold transition cursor-pointer ${
              activeTab === "claude"
                ? "bg-[var(--card-bg)] shadow-xs text-[var(--text-main)]"
                : "text-[var(--text-muted)] hover:text-[var(--text-main)]"
            }`}
          >
            <BrandIcon provider="claude" size={13} />
            <span>Claude</span>
            <span className="text-[10px] opacity-80 tabular-nums">
              {data.claude.five_hour.used_pct.toFixed(0)}%
            </span>
          </button>

          <button
            type="button"
            onClick={() => onSelectTab("agy")}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-semibold transition cursor-pointer ${
              activeTab === "agy"
                ? "bg-[var(--card-bg)] shadow-xs text-[var(--text-main)]"
                : "text-[var(--text-muted)] hover:text-[var(--text-main)]"
            }`}
          >
            <BrandIcon provider="agy" size={13} />
            <span>AGY</span>
            <span className="text-[10px] opacity-80 tabular-nums">
              {data.agy.five_hour.used_pct.toFixed(0)}%
            </span>
          </button>
        </div>

        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={handleManualRefresh}
            title="รีเฟรชข้อมูลทันที"
            aria-label="รีเฟรชข้อมูลทันที"
            className="p-1.5 rounded-lg text-[var(--text-muted)] hover:text-[var(--text-main)] hover:bg-[var(--meter-track)] cursor-pointer"
          >
            <RefreshCw
              size={14}
              aria-hidden="true"
              className={refreshing ? "animate-spin" : ""}
            />
          </button>

          <button
            type="button"
            onClick={() =>
              onSelectTab(activeTab === "settings" ? activeProvider : "settings")
            }
            title="ตั้งค่า"
            aria-label="ตั้งค่า"
            className={`p-1.5 rounded-lg cursor-pointer ${
              activeTab === "settings"
                ? "bg-[var(--card-bg)] text-[var(--text-main)] border border-[var(--card-border)]"
                : "text-[var(--text-muted)] hover:text-[var(--text-main)] hover:bg-[var(--meter-track)]"
            }`}
          >
            <Settings size={14} aria-hidden="true" />
          </button>
        </div>
      </div>

      {/* Scrollable Content */}
      <div className="scroll-area flex-1 overflow-y-auto p-3 space-y-3">
        {activeTab === "settings" ? (
          <SettingsPanel config={data.config} onUpdated={onDataUpdated} />
        ) : (
          <>
            {/* Active Model & Source Subtitle */}
            <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)] px-0.5">
              <span className="truncate">
                โมเดลล่าสุด:{" "}
                <strong className="text-[var(--text-main)]">
                  {report.active_model ?? "ยังไม่มีข้อมูล"}
                </strong>
              </span>
              <span className="shrink-0 text-[10px] px-1.5 py-0.5 rounded bg-[var(--meter-track)]">
                {report.five_hour.source === "official"
                  ? "Official Quota"
                  : "Local 5h Block"}
              </span>
            </div>

            {/* 5h & Weekly Quota Gauges */}
            <div className="glass-card p-3 space-y-3">
              <QuotaBar
                title="5-Hour Limit"
                window={report.five_hour}
                provider={activeProvider}
                nowEpochSec={nowEpochSec}
              />
              <div className="border-t border-[var(--card-border)] pt-2.5">
                <QuotaBar
                  title="Weekly Limit (7 วัน)"
                  window={report.weekly}
                  provider={activeProvider}
                  nowEpochSec={nowEpochSec}
                  isWeekly
                />
              </div>
            </div>

            {/* Time Range Filter Pills */}
            <div className="flex items-center justify-between gap-1 p-0.5 rounded-lg bg-[var(--meter-track)]">
              {RANGE_OPTIONS.map((opt) => (
                <button
                  key={opt.key}
                  type="button"
                  onClick={() => setRangeKey(opt.key)}
                  className={`flex-1 py-1 rounded-md text-[11px] font-medium transition cursor-pointer ${
                    rangeKey === opt.key
                      ? "bg-[var(--card-bg)] text-[var(--text-main)] shadow-xs"
                      : "text-[var(--text-muted)] hover:text-[var(--text-main)]"
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>

            {/* Token Breakdown Grid */}
            <TokenGrid tokens={currentRangeStats.tokens} />

            {/* Trend Chart */}
            <UsageChart
              provider={activeProvider}
              rangeKey={rangeKey}
              hourly24h={report.hourly_24h}
              daily30d={report.daily_30d}
            />

            {/* Per-Model Breakdown */}
            <ModelBreakdown
              models={currentRangeStats.models}
              provider={activeProvider}
            />
          </>
        )}
      </div>
    </div>
  );
}

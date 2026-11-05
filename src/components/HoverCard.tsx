import { FullUsagePayload, getStatusMeta, ProviderId } from "../types/usage";
import { BrandIcon } from "./BrandIcon";
import { QuotaBar } from "./QuotaBar";

interface HoverCardProps {
  data: FullUsagePayload | null;
  provider: ProviderId;
  nowEpochSec: number;
}

export function HoverCard({ data, provider, nowEpochSec }: HoverCardProps) {
  if (!data) {
    return (
      <div className="popover-shell w-full h-full p-3 flex items-center justify-center text-xs text-[var(--text-muted)]">
        กำลังโหลดข้อมูล...
      </div>
    );
  }

  const report = provider === "claude" ? data.claude : data.agy;
  const displayMode = data.config.quota_display_mode;
  const maxUsed = Math.max(report.five_hour.used_pct, report.weekly.used_pct);
  const status = getStatusMeta(maxUsed, provider, displayMode);

  return (
    <div className="popover-shell w-full h-full p-3 flex flex-col justify-between">
      <div className="flex items-center justify-between gap-2 pb-1.5 border-b border-[var(--card-border)]">
        <div className="flex items-center gap-1.5 min-w-0">
          <BrandIcon provider={provider} size={15} />
          <span className="text-xs font-bold tracking-tight">{report.name}</span>
          {report.active_model && (
            <span className="text-[10px] text-[var(--text-muted)] truncate">
              · {report.active_model}
            </span>
          )}
        </div>
        <span
          className={`px-1.5 py-0.5 rounded-full text-[10px] font-medium shrink-0 ${status.badgeClass}`}
        >
          {status.badge}
        </span>
      </div>

      <div className="space-y-2.5 pt-1">
        <QuotaBar
          title="5-Hour Limit"
          window={report.five_hour}
          provider={provider}
          nowEpochSec={nowEpochSec}
          displayMode={displayMode}
          compact
        />
        <QuotaBar
          title="Weekly Limit (7d)"
          window={report.weekly}
          provider={provider}
          nowEpochSec={nowEpochSec}
          displayMode={displayMode}
          isWeekly
          compact
        />
      </div>
    </div>
  );
}

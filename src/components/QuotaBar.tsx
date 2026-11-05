import { CSSProperties, useId } from "react";
import { AlertTriangle, CheckCircle2, OctagonAlert } from "lucide-react";
import {
  formatResetLabel,
  formatTokens,
  getStatusMeta,
  ProviderId,
  QuotaWindowStatus,
} from "../types/usage";

interface QuotaBarProps {
  title: string;
  window: QuotaWindowStatus;
  provider: ProviderId;
  nowEpochSec: number;
  isWeekly?: boolean;
  compact?: boolean;
}

export function QuotaBar({
  title,
  window,
  provider,
  nowEpochSec,
  isWeekly = false,
  compact = false,
}: QuotaBarProps) {
  const labelId = useId();
  const status = getStatusMeta(window.used_pct, provider);
  const clampedValue = Math.min(100, Math.max(0, window.used_pct));

  const StatusIcon =
    status.level === "critical"
      ? OctagonAlert
      : status.level === "warning"
        ? AlertTriangle
        : CheckCircle2;

  return (
    <div className={compact ? "space-y-1.5" : "space-y-2"}>
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5 min-w-0">
          <span
            id={labelId}
            className="text-xs font-semibold tracking-tight truncate"
          >
            {title}
          </span>
          {!compact && (
            <span
              className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[10px] font-medium ${status.badgeClass}`}
            >
              <StatusIcon size={10} aria-hidden="true" />
              <span>{status.badge}</span>
            </span>
          )}
        </div>

        <div className="flex items-baseline gap-1.5 shrink-0">
          {compact && status.level !== "normal" && (
            <StatusIcon
              size={11}
              aria-hidden="true"
              className={status.textClass}
            />
          )}
          <span className={`text-xs font-bold tabular-nums ${status.textClass}`}>
            {window.used_pct.toFixed(window.used_pct < 10 && window.used_pct > 0 ? 1 : 0)}%
          </span>
        </div>
      </div>

      <progress
        className="usage-meter"
        max={100}
        value={clampedValue}
        aria-labelledby={labelId}
        style={
          {
            "--progress-bar-fill": status.fill,
            "--progress-height": compact ? "6px" : "8px",
          } as CSSProperties
        }
      />

      <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)]">
        <span className="truncate">
          {formatResetLabel(window.resets_at, nowEpochSec, isWeekly)}
        </span>
        {!compact && window.limit_tokens ? (
          <span className="tabular-nums shrink-0">
            {formatTokens(window.used_tokens)} / {formatTokens(window.limit_tokens)}
          </span>
        ) : !compact && window.used_tokens > 0 ? (
          <span className="tabular-nums shrink-0">
            {formatTokens(window.used_tokens)} active
          </span>
        ) : null}
      </div>
    </div>
  );
}

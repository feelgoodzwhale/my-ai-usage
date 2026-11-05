import { useState } from "react";
import {
  formatTokens,
  ProviderId,
  RangeKey,
  TrendBucket,
} from "../types/usage";

interface UsageChartProps {
  provider: ProviderId;
  rangeKey: RangeKey;
  hourly24h: TrendBucket[];
  daily30d: TrendBucket[];
}

export function UsageChart({
  provider,
  rangeKey,
  hourly24h,
  daily30d,
}: UsageChartProps) {
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  const isHourly = rangeKey === "five_hour" || rangeKey === "today";
  const buckets = isHourly
    ? hourly24h
    : rangeKey === "seven_day"
      ? daily30d.slice(-7)
      : daily30d;

  const maxActive = Math.max(1, ...buckets.map((b) => b.active_tokens));
  const barColor = provider === "claude" ? "#f97316" : "#38bdf8";

  const activeBucket =
    hoveredIdx !== null && buckets[hoveredIdx]
      ? buckets[hoveredIdx]
      : buckets[buckets.length - 1];

  return (
    <div className="glass-card p-2.5 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-semibold">
          {isHourly
            ? "แนวโน้มรายชั่วโมง (24 ชม.)"
            : rangeKey === "seven_day"
              ? "แนวโน้มรายวัน (7 วัน)"
              : "แนวโน้มรายวัน (30 วัน)"}
        </span>
        {activeBucket && (
          <span className="text-[10px] text-[var(--text-muted)] tabular-nums">
            {activeBucket.label}:{" "}
            <strong className="text-[var(--text-main)]">
              {formatTokens(activeBucket.active_tokens)}
            </strong>{" "}
            (Cache {formatTokens(activeBucket.cache_read_tokens)})
          </span>
        )}
      </div>

      <div
        className="h-24 flex items-end gap-[3px] pt-2"
        onMouseLeave={() => setHoveredIdx(null)}
      >
        {buckets.map((b, idx) => {
          const heightPct =
            b.active_tokens > 0
              ? Math.max(8, (b.active_tokens / maxActive) * 100)
              : 3;
          const isHovered = hoveredIdx === idx;
          return (
            <div
              key={`${b.timestamp}-${idx}`}
              className="flex-1 h-full flex flex-col justify-end items-center group cursor-pointer"
              onMouseEnter={() => setHoveredIdx(idx)}
            >
              <div
                className="w-full rounded-t-sm transition-opacity"
                style={{
                  height: `${heightPct}%`,
                  backgroundColor:
                    b.active_tokens > 0 ? barColor : "var(--meter-track)",
                  opacity:
                    hoveredIdx === null ? 0.85 : isHovered ? 1 : 0.4,
                }}
              />
            </div>
          );
        })}
      </div>

      {buckets.length > 0 && (
        <div className="flex items-center justify-between text-[10px] text-[var(--text-muted)] tabular-nums">
          <span>{buckets[0]?.label}</span>
          <span>{buckets[Math.floor(buckets.length / 2)]?.label}</span>
          <span>{buckets[buckets.length - 1]?.label}</span>
        </div>
      )}
    </div>
  );
}

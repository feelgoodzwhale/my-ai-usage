import { CSSProperties } from "react";
import { Cpu } from "lucide-react";
import { formatTokens, ModelUsageItem, ProviderId } from "../types/usage";

interface ModelBreakdownProps {
  models: ModelUsageItem[];
  provider: ProviderId;
}

export function ModelBreakdown({ models, provider }: ModelBreakdownProps) {
  const fill = provider === "claude" ? "#fb923c" : "#60a5fa";

  return (
    <div className="glass-card p-2.5 space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5 text-xs font-semibold">
          <Cpu size={13} aria-hidden="true" className="text-[var(--text-muted)]" />
          <span>แยกตามรุ่นโมเดล</span>
        </div>
        <span className="text-[10px] text-[var(--text-muted)]">
          {models.length} โมเดล
        </span>
      </div>

      {models.length === 0 ? (
        <div className="text-[11px] text-[var(--text-muted)] py-2 text-center">
          ยังไม่มีการใช้งานในช่วงเวลานี้
        </div>
      ) : (
        <div className="space-y-2">
          {models.slice(0, 6).map((m, idx) => {
            const labelId = `model-bar-${provider}-${idx}`;
            return (
              <div key={m.model} className="space-y-1">
                <div className="flex items-center justify-between gap-2 text-[11px]">
                  <span id={labelId} className="font-medium truncate">
                    {m.model}
                  </span>
                  <span className="tabular-nums text-[var(--text-muted)] shrink-0">
                    <strong className="text-[var(--text-main)]">
                      {formatTokens(m.active_tokens)}
                    </strong>{" "}
                    ({m.share_pct.toFixed(0)}%)
                  </span>
                </div>
                <progress
                  className="usage-meter"
                  max={100}
                  value={Math.min(100, Math.max(0, m.share_pct))}
                  aria-labelledby={labelId}
                  style={
                    {
                      "--progress-bar-fill": fill,
                      "--progress-height": "5px",
                    } as CSSProperties
                  }
                />
                {m.cache_read_tokens > 0 && (
                  <div className="flex justify-between text-[10px] text-[var(--text-muted)] tabular-nums">
                    <span>
                      In {formatTokens(m.input_tokens)} · Out{" "}
                      {formatTokens(m.output_tokens)}
                    </span>
                    <span>Cache {formatTokens(m.cache_read_tokens)}</span>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

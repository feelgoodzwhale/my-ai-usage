export type ProviderId = "claude" | "agy";
export type RangeKey = "five_hour" | "today" | "seven_day" | "thirty_day";
export type AgyQuotaMode = "active_only" | "weighted_cache" | "include_all_cache";

export interface QuotaWindowStatus {
  used_pct: number;
  used_tokens: number;
  limit_tokens: number | null;
  resets_at: number | null;
  source: "official" | "local_block" | string;
}

export interface TokenSummary {
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  active_tokens: number;
  total_tokens: number;
  message_count: number;
}

export interface ModelUsageItem {
  model: string;
  active_tokens: number;
  total_tokens: number;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  share_pct: number;
}

export interface RangeStats {
  tokens: TokenSummary;
  models: ModelUsageItem[];
}

export interface TrendBucket {
  label: string;
  timestamp: number;
  active_tokens: number;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
}

export interface ProviderRanges {
  five_hour: RangeStats;
  today: RangeStats;
  seven_day: RangeStats;
  thirty_day: RangeStats;
}

export interface ProviderUsageReport {
  id: string;
  name: string;
  plan_label: string;
  active_model: string | null;
  five_hour: QuotaWindowStatus;
  weekly: QuotaWindowStatus;
  ranges: ProviderRanges;
  hourly_24h: TrendBucket[];
  daily_30d: TrendBucket[];
  last_updated: number;
}

export interface AppConfig {
  agy_five_hour_limit: number;
  agy_weekly_limit: number;
  claude_fallback_five_hour_limit: number;
  claude_fallback_weekly_limit: number;
  agy_quota_mode: AgyQuotaMode;
  notify_enabled: boolean;
  notify_warn_pct: number;
  notify_crit_pct: number;
  notify_on_reset: boolean;
}

export interface FullUsagePayload {
  claude: ProviderUsageReport;
  agy: ProviderUsageReport;
  config: AppConfig;
  generated_at: number;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(2)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 10_000) return `${(n / 1_000).toFixed(1)}K`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(2)}K`;
  return n.toLocaleString();
}

export function formatResetLabel(
  resetsAt: number | null,
  nowEpochSec: number,
  isWeekly = false
): string {
  if (!resetsAt) {
    return "พร้อมเริ่มรอบใหม่";
  }
  const diff = resetsAt - nowEpochSec;
  if (diff <= 0) {
    return "รีเซ็ตแล้ว (0%)";
  }

  const d = new Date(resetsAt * 1000);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");

  if (isWeekly) {
    const days = Math.floor(diff / 86400);
    const hours = Math.floor((diff % 86400) / 3600);
    const weekday = d.toLocaleDateString("th-TH", { weekday: "short" });
    const countdown = days > 0 ? `${days}d ${hours}h` : `${hours}h`;
    return `Reset ${weekday} ${hh}:${mm} (${countdown})`;
  }

  const hours = Math.floor(diff / 3600);
  const mins = Math.floor((diff % 3600) / 60);
  const countdown = hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
  return `Reset ${hh}:${mm} (อีก ${countdown})`;
}

export function getStatusMeta(pct: number, provider: ProviderId) {
  if (pct >= 90) {
    return {
      level: "critical" as const,
      badge: "ใกล้เต็ม",
      fill: "#ef4444",
      textClass: "text-red-500 dark:text-red-400",
      badgeClass:
        "bg-red-500/15 text-red-600 dark:text-red-300 border border-red-500/35",
    };
  }
  if (pct >= 75) {
    return {
      level: "warning" as const,
      badge: "เริ่มตึง",
      fill: "#f59e0b",
      textClass: "text-amber-600 dark:text-amber-400",
      badgeClass:
        "bg-amber-500/15 text-amber-700 dark:text-amber-300 border border-amber-500/35",
    };
  }
  const brandFill = provider === "claude" ? "#f97316" : "#38bdf8";
  return {
    level: "normal" as const,
    badge: "ปกติ",
    fill: brandFill,
    textClass:
      provider === "claude"
        ? "text-orange-600 dark:text-orange-400"
        : "text-sky-600 dark:text-sky-400",
    badgeClass:
      provider === "claude"
        ? "bg-orange-500/12 text-orange-700 dark:text-orange-300 border border-orange-500/30"
        : "bg-sky-500/12 text-sky-700 dark:text-sky-300 border border-sky-500/30",
  };
}

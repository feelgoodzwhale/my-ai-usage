import { ArrowDownLeft, ArrowUpRight, Database, Layers } from "lucide-react";
import { formatTokens, TokenSummary } from "../types/usage";

export function TokenGrid({ tokens }: { tokens: TokenSummary }) {
  const cards = [
    {
      label: "Input Tokens",
      value: formatTokens(tokens.input_tokens),
      raw: tokens.input_tokens.toLocaleString(),
      icon: ArrowUpRight,
      accent: "text-sky-500 dark:text-sky-400",
    },
    {
      label: "Output Tokens",
      value: formatTokens(tokens.output_tokens),
      raw: tokens.output_tokens.toLocaleString(),
      icon: ArrowDownLeft,
      accent: "text-emerald-500 dark:text-emerald-400",
    },
    {
      label: "Cache Read",
      value: formatTokens(tokens.cache_read_tokens),
      raw: tokens.cache_read_tokens.toLocaleString(),
      icon: Database,
      accent: "text-violet-500 dark:text-violet-400",
    },
    {
      label: "Cache Write",
      value: formatTokens(tokens.cache_write_tokens),
      raw: tokens.cache_write_tokens.toLocaleString(),
      icon: Layers,
      accent: "text-amber-500 dark:text-amber-400",
    },
  ];

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-2 gap-2">
        {cards.map((item) => {
          const Icon = item.icon;
          return (
            <div
              key={item.label}
              className="glass-card px-2.5 py-2 flex flex-col justify-between"
              title={`${item.label}: ${item.raw} tokens`}
            >
              <div className="flex items-center justify-between text-[11px] text-[var(--text-muted)]">
                <span>{item.label}</span>
                <Icon size={12} aria-hidden="true" className={item.accent} />
              </div>
              <div className="text-sm font-bold tabular-nums mt-0.5">
                {item.value}
              </div>
            </div>
          );
        })}
      </div>

      <div className="glass-card px-2.5 py-1.5 flex items-center justify-between text-[11px] text-[var(--text-muted)]">
        <span>
          Active (In+Out):{" "}
          <strong className="text-[var(--text-main)] tabular-nums">
            {formatTokens(tokens.active_tokens)}
          </strong>
        </span>
        <span>
          รวมทั้งหมด:{" "}
          <strong className="text-[var(--text-main)] tabular-nums">
            {formatTokens(tokens.total_tokens)}
          </strong>{" "}
          ({tokens.message_count.toLocaleString()} steps)
        </span>
      </div>
    </div>
  );
}

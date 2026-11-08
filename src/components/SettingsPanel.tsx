import { FormEvent, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { Check, Power, Save } from "lucide-react";
import {
  AgyQuotaMode,
  AppConfig,
  FullUsagePayload,
  QuotaDisplayMode,
} from "../types/usage";

interface SettingsPanelProps {
  config: AppConfig;
  onUpdated: (payload: FullUsagePayload) => void;
}

export function SettingsPanel({ config, onUpdated }: SettingsPanelProps) {
  const [draft, setDraft] = useState<AppConfig>(config);
  const [autostart, setAutostart] = useState(false);
  const [savedToast, setSavedToast] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  useEffect(() => {
    isAutostartEnabled()
      .then(setAutostart)
      .catch(() => {});
  }, []);

  const toggleAutostart = async (next: boolean) => {
    try {
      if (next) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
      setAutostart(next);
    } catch {
      // Ignore in dev mode if bundle registration not yet active
    }
  };

  const persistConfig = async (nextConfig: AppConfig, showToast = true) => {
    setIsSubmitting(true);
    try {
      const updated = await invoke<FullUsagePayload>("save_app_config", {
        config: nextConfig,
      });
      onUpdated(updated);
      if (showToast) {
        setSavedToast(true);
        setTimeout(() => setSavedToast(false), 1800);
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDisplayModeChange = (mode: QuotaDisplayMode) => {
    const next = { ...draft, quota_display_mode: mode };
    setDraft(next);
    void persistConfig(next, false);
  };

  const handleSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    void persistConfig(draft, true);
  };

  const handleQuit = async () => {
    await invoke("quit_app");
  };

  const presets5h = [
    { label: "1M", val: 1_000_000 },
    { label: "3M", val: 3_000_000 },
    { label: "5M", val: 5_000_000 },
    { label: "10M", val: 10_000_000 },
  ];

  return (
    <form onSubmit={handleSubmit} className="space-y-3 pb-1">
      <fieldset className="glass-card p-3 space-y-2 border-0 m-0 min-w-0">
        <legend className="text-xs font-semibold p-0 mb-1.5">
          รูปแบบการแสดงผลโควต้า (Quota Display)
        </legend>

        <label
          htmlFor="display-mode-remaining"
          className={`flex items-start gap-2.5 p-2 rounded-lg border text-xs cursor-pointer transition ${
            draft.quota_display_mode === "remaining"
              ? "border-sky-500/50 bg-sky-500/10"
              : "border-[var(--card-border)] hover:bg-[var(--meter-track)]"
          }`}
        >
          <input
            id="display-mode-remaining"
            type="radio"
            name="quota_display_mode"
            value="remaining"
            checked={draft.quota_display_mode === "remaining"}
            onChange={() => handleDisplayModeChange("remaining")}
            aria-describedby="desc-mode-remaining"
            className="mt-0.5 shrink-0"
          />
          <span className="space-y-0.5">
            <span className="block font-semibold text-[var(--text-main)]">
              คงเหลือเท่าไร (Remaining %) · ค่าเริ่มต้น
            </span>
            <span
              id="desc-mode-remaining"
              className="block text-[11px] text-[var(--text-muted)]"
            >
              จาก 100% ตอนนี้เหลือเท่าไร (เช่น 98.4%)
            </span>
          </span>
        </label>

        <label
          htmlFor="display-mode-used"
          className={`flex items-start gap-2.5 p-2 rounded-lg border text-xs cursor-pointer transition ${
            draft.quota_display_mode === "used"
              ? "border-sky-500/50 bg-sky-500/10"
              : "border-[var(--card-border)] hover:bg-[var(--meter-track)]"
          }`}
        >
          <input
            id="display-mode-used"
            type="radio"
            name="quota_display_mode"
            value="used"
            checked={draft.quota_display_mode === "used"}
            onChange={() => handleDisplayModeChange("used")}
            aria-describedby="desc-mode-used"
            className="mt-0.5 shrink-0"
          />
          <span className="space-y-0.5">
            <span className="block font-semibold text-[var(--text-main)]">
              ใช้ไปเท่าไร (Used %)
            </span>
            <span
              id="desc-mode-used"
              className="block text-[11px] text-[var(--text-muted)]"
            >
              ใช้ไปแล้วกี่เปอร์เซ็นต์จากโควต้า (เช่น 1.6%)
            </span>
          </span>
        </label>

        <p className="text-[10px] text-[var(--text-muted)] pt-0.5 leading-relaxed">
          บน Menu Bar จะแสดงเฉพาะโควต้ารอบ 5 ชั่วโมงเท่านั้น ส่วน Weekly Limit
          ดูได้ทันทีเมื่อนำเมาส์ไปชี้ (Hover) หรือคลิกเปิดแอพ
        </p>
      </fieldset>

      <fieldset className="glass-card p-3 space-y-2.5 border-0 m-0 min-w-0">
        <legend className="text-xs font-semibold p-0 mb-1">
          เพดาน Claude Cost (เมื่อใช้งานผ่าน Claude Desktop)
        </legend>

        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1">
            <label
              htmlFor="claude-5h-usd"
              className="block text-[11px] text-[var(--text-muted)]"
            >
              งบรอบ 5 ชั่วโมง (USD)
            </label>
            <input
              id="claude-5h-usd"
              name="claude_five_hour_budget_usd"
              type="number"
              min={1}
              step={0.5}
              value={draft.claude_five_hour_budget_usd}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  claude_five_hour_budget_usd: Math.max(
                    1,
                    Number(e.target.value) || 0
                  ),
                })
              }
              className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-[var(--card-border)] bg-transparent tabular-nums"
            />
          </div>

          <div className="space-y-1">
            <label
              htmlFor="claude-7d-usd"
              className="block text-[11px] text-[var(--text-muted)]"
            >
              งบรายสัปดาห์ 7 วัน (USD)
            </label>
            <input
              id="claude-7d-usd"
              name="claude_weekly_budget_usd"
              type="number"
              min={5}
              step={2.5}
              value={draft.claude_weekly_budget_usd}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  claude_weekly_budget_usd: Math.max(
                    5,
                    Number(e.target.value) || 0
                  ),
                })
              }
              className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-[var(--card-border)] bg-transparent tabular-nums"
            />
          </div>
        </div>

        <div className="flex gap-1.5 pt-0.5">
          {[
            { label: "Pro", b5: 12, b7: 85 },
            { label: "Max 5x", b5: 38.5, b7: 437.5 },
            { label: "Max 20x", b5: 150, b7: 1750 },
          ].map((tier) => (
            <button
              key={tier.label}
              type="button"
              onClick={() =>
                setDraft({
                  ...draft,
                  claude_five_hour_budget_usd: tier.b5,
                  claude_weekly_budget_usd: tier.b7,
                })
              }
              className="px-2 py-0.5 text-[10px] rounded-md border border-[var(--card-border)] hover:opacity-80 cursor-pointer"
            >
              {tier.label}
            </button>
          ))}
        </div>
      </fieldset>

      <fieldset className="glass-card p-3 space-y-2.5 border-0 m-0 min-w-0">
        <legend className="text-xs font-semibold p-0 mb-1">
          เพดาน Token สำรอง AGY (กรณีไม่มี Official Quota)
        </legend>

        <div className="space-y-1">
          <label
            htmlFor="agy-5h-limit"
            className="flex justify-between text-[11px] text-[var(--text-muted)]"
          >
            <span>เพดาน AGY รอบ 5 ชั่วโมง (Tokens)</span>
            <span className="tabular-nums font-medium text-[var(--text-main)]">
              {draft.agy_five_hour_limit.toLocaleString()}
            </span>
          </label>
          <input
            id="agy-5h-limit"
            name="agy_five_hour_limit"
            type="number"
            min={10_000}
            step={50_000}
            value={draft.agy_five_hour_limit}
            onChange={(e) =>
              setDraft({
                ...draft,
                agy_five_hour_limit: Math.max(10_000, Number(e.target.value) || 0),
              })
            }
            className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-[var(--card-border)] bg-transparent tabular-nums"
          />
          <div className="flex gap-1.5 pt-0.5">
            {presets5h.map((p) => (
              <button
                key={p.label}
                type="button"
                onClick={() =>
                  setDraft({ ...draft, agy_five_hour_limit: p.val })
                }
                className="px-2 py-0.5 text-[10px] rounded-md border border-[var(--card-border)] hover:opacity-80 cursor-pointer"
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>

        <div className="space-y-1">
          <label
            htmlFor="agy-7d-limit"
            className="flex justify-between text-[11px] text-[var(--text-muted)]"
          >
            <span>เพดาน AGY รายสัปดาห์ 7 วัน (Tokens)</span>
            <span className="tabular-nums font-medium text-[var(--text-main)]">
              {draft.agy_weekly_limit.toLocaleString()}
            </span>
          </label>
          <input
            id="agy-7d-limit"
            name="agy_weekly_limit"
            type="number"
            min={50_000}
            step={250_000}
            value={draft.agy_weekly_limit}
            onChange={(e) =>
              setDraft({
                ...draft,
                agy_weekly_limit: Math.max(50_000, Number(e.target.value) || 0),
              })
            }
            className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-[var(--card-border)] bg-transparent tabular-nums"
          />
        </div>

        <div className="space-y-1">
          <label
            htmlFor="agy-quota-mode"
            className="block text-[11px] text-[var(--text-muted)]"
          >
            วิธีคำนวณ Token สำรอง
          </label>
          <select
            id="agy-quota-mode"
            name="agy_quota_mode"
            value={draft.agy_quota_mode}
            onChange={(e) =>
              setDraft({
                ...draft,
                agy_quota_mode: e.target.value as AgyQuotaMode,
              })
            }
            className="w-full px-2 py-1.5 text-xs rounded-lg border border-[var(--card-border)] bg-[var(--card-bg)]"
          >
            <option value="active_only">
              Active Tokens เท่านั้น (Input + Output)
            </option>
            <option value="weighted_cache">
              ถ่วงน้ำหนัก Cache (In + Out + 10% Cache Read)
            </option>
            <option value="include_all_cache">
              นับรวมทุก Token 100% (รวม Cache ทั้งหมด)
            </option>
          </select>
        </div>
      </fieldset>

      <fieldset className="glass-card p-3 space-y-2 border-0 m-0 min-w-0">
        <legend className="text-xs font-semibold p-0 mb-1">
          ระบบแจ้งเตือน & macOS
        </legend>

        <label
          htmlFor="notify-enabled"
          className="flex items-center justify-between text-xs cursor-pointer"
        >
          <span>แจ้งเตือนเมื่อโควต้าแตะ 80% / 95% (เหลือ 20% / 5%)</span>
          <input
            id="notify-enabled"
            name="notify_enabled"
            type="checkbox"
            checked={draft.notify_enabled}
            onChange={(e) =>
              setDraft({ ...draft, notify_enabled: e.target.checked })
            }
          />
        </label>

        <label
          htmlFor="notify-on-reset"
          className="flex items-center justify-between text-xs cursor-pointer"
        >
          <span>แจ้งเตือนเมื่อครบรอบ Reset โควต้าเต็ม 100%</span>
          <input
            id="notify-on-reset"
            name="notify_on_reset"
            type="checkbox"
            checked={draft.notify_on_reset}
            onChange={(e) =>
              setDraft({ ...draft, notify_on_reset: e.target.checked })
            }
          />
        </label>

        <label
          htmlFor="autostart-enabled"
          className="flex items-center justify-between text-xs cursor-pointer"
        >
          <span>เปิดแอพอัตโนมัติเมื่อเปิดเครื่อง (Launch at Login)</span>
          <input
            id="autostart-enabled"
            name="autostart_enabled"
            type="checkbox"
            checked={autostart}
            onChange={(e) => toggleAutostart(e.target.checked)}
          />
        </label>
      </fieldset>

      <div className="flex items-center justify-between gap-2 pt-0.5">
        <button
          type="button"
          onClick={handleQuit}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-red-600 dark:text-red-400 border border-red-500/30 hover:bg-red-500/10 cursor-pointer"
        >
          <Power size={13} aria-hidden="true" />
          <span>ออกจากแอพ</span>
        </button>

        <button
          type="submit"
          disabled={isSubmitting}
          className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-semibold bg-sky-500 text-white hover:bg-sky-600 disabled:opacity-60 cursor-pointer"
        >
          {savedToast ? (
            <>
              <Check size={13} aria-hidden="true" />
              <span>บันทึกแล้ว</span>
            </>
          ) : (
            <>
              <Save size={13} aria-hidden="true" />
              <span>บันทึกการตั้งค่า</span>
            </>
          )}
        </button>
      </div>
    </form>
  );
}

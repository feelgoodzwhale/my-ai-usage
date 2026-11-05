import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { Check, Power, Save } from "lucide-react";
import { AgyQuotaMode, AppConfig, FullUsagePayload } from "../types/usage";

interface SettingsPanelProps {
  config: AppConfig;
  onUpdated: (payload: FullUsagePayload) => void;
}

export function SettingsPanel({ config, onUpdated }: SettingsPanelProps) {
  const [draft, setDraft] = useState<AppConfig>(config);
  const [autostart, setAutostart] = useState(false);
  const [savedToast, setSavedToast] = useState(false);

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

  const handleSave = async () => {
    const updated = await invoke<FullUsagePayload>("save_app_config", {
      config: draft,
    });
    onUpdated(updated);
    setSavedToast(true);
    setTimeout(() => setSavedToast(false), 1800);
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
    <div className="space-y-3 pb-1">
      <div className="glass-card p-3 space-y-2.5">
        <div className="text-xs font-semibold">เพดานโควต้า AGY (Token Budget)</div>

        <div className="space-y-1">
          <label
            htmlFor="agy-5h-limit"
            className="flex justify-between text-[11px] text-[var(--text-muted)]"
          >
            <span>เพดานรอบ 5 ชั่วโมง (Tokens)</span>
            <span className="tabular-nums font-medium text-[var(--text-main)]">
              {draft.agy_five_hour_limit.toLocaleString()}
            </span>
          </label>
          <input
            id="agy-5h-limit"
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
            <span>เพดานรายสัปดาห์ 7 วัน (Tokens)</span>
            <span className="tabular-nums font-medium text-[var(--text-main)]">
              {draft.agy_weekly_limit.toLocaleString()}
            </span>
          </label>
          <input
            id="agy-7d-limit"
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
            วิธีคำนวณ % โควต้า AGY
          </label>
          <select
            id="agy-quota-mode"
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
      </div>

      <div className="glass-card p-3 space-y-2">
        <div className="text-xs font-semibold">ระบบแจ้งเตือน & macOS</div>

        <label className="flex items-center justify-between text-xs cursor-pointer">
          <span>แจ้งเตือนเมื่อโควต้าแตะ 80% / 95%</span>
          <input
            type="checkbox"
            checked={draft.notify_enabled}
            onChange={(e) =>
              setDraft({ ...draft, notify_enabled: e.target.checked })
            }
          />
        </label>

        <label className="flex items-center justify-between text-xs cursor-pointer">
          <span>แจ้งเตือนเมื่อครบรอบ Reset เป็น 0%</span>
          <input
            type="checkbox"
            checked={draft.notify_on_reset}
            onChange={(e) =>
              setDraft({ ...draft, notify_on_reset: e.target.checked })
            }
          />
        </label>

        <label className="flex items-center justify-between text-xs cursor-pointer">
          <span>เปิดแอพอัตโนมัติเมื่อเปิดเครื่อง (Launch at Login)</span>
          <input
            type="checkbox"
            checked={autostart}
            onChange={(e) => toggleAutostart(e.target.checked)}
          />
        </label>
      </div>

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
          type="button"
          onClick={handleSave}
          className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-semibold bg-sky-500 text-white hover:bg-sky-600 cursor-pointer"
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
    </div>
  );
}

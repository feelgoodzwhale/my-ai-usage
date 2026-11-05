import { ProviderId } from "../types/usage";

export function BrandIcon({
  provider,
  size = 16,
}: {
  provider: ProviderId;
  size?: number;
}) {
  if (provider === "claude") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true"
        className="shrink-0 text-orange-500 dark:text-orange-400"
      >
        <g stroke="currentColor" strokeWidth="2.1" strokeLinecap="round">
          <line x1="12" y1="2.5" x2="12" y2="7.5" />
          <line x1="12" y1="16.5" x2="12" y2="21.5" />
          <line x1="2.5" y1="12" x2="7.5" y2="12" />
          <line x1="16.5" y1="12" x2="21.5" y2="12" />
          <line x1="5.3" y1="5.3" x2="8.8" y2="8.8" />
          <line x1="15.2" y1="15.2" x2="18.7" y2="18.7" />
          <line x1="18.7" y1="5.3" x2="15.2" y2="8.8" />
          <line x1="8.8" y1="15.2" x2="5.3" y2="18.7" />
        </g>
        <circle cx="12" cy="12" r="2.2" fill="currentColor" />
      </svg>
    );
  }

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      className="shrink-0 text-sky-500 dark:text-sky-400"
    >
      <path
        d="M12 3.2L4.2 19.2L8.8 16.4M12 3.2L19.8 19.2L15.2 16.4"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="12" cy="13.4" r="2.3" fill="currentColor" />
    </svg>
  );
}

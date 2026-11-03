export function LogoMark({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden>
      <defs>
        <linearGradient id="dl-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#6E6BFF" />
          <stop offset="1" stopColor="#2BB8F0" />
        </linearGradient>
      </defs>
      <rect x="40" y="40" width="944" height="944" rx="230" fill="url(#dl-bg)" />
      <g fill="none" stroke="#fff" strokeWidth="84" strokeLinecap="round" strokeLinejoin="round">
        <path d="M310 574 L604 280 M434 280 H604 V450" />
        <path d="M714 450 L420 744 M590 744 H420 V574" opacity="0.92" />
      </g>
    </svg>
  );
}

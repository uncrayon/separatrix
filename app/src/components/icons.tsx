// Small inline SVG icons (16 px grid, stroke = currentColor).

import type { ReactNode, SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 16, children, ...rest }: IconProps & { children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const IconGallery = (p: IconProps) => (
  <Svg {...p}>
    <rect x="2" y="2" width="5" height="5" rx="1.2" />
    <rect x="9" y="2" width="5" height="5" rx="1.2" />
    <rect x="2" y="9" width="5" height="5" rx="1.2" />
    <rect x="9" y="9" width="5" height="5" rx="1.2" />
  </Svg>
);

export const IconProjector = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="3" />
    <path d="M8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1" />
  </Svg>
);

export const IconMoon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M13 9.5A5.5 5.5 0 0 1 6.5 3a5.5 5.5 0 1 0 6.5 6.5Z" />
  </Svg>
);

export const IconPlus = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 3v10M3 8h10" />
  </Svg>
);

export const IconClose = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 4l8 8M12 4l-8 8" />
  </Svg>
);

export const IconChevron = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4.5 6.5 8 10l3.5-3.5" />
  </Svg>
);

export const IconReset = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.5 8a5.5 5.5 0 1 0 1.7-4" />
    <path d="M2.5 2.5V5h2.5" />
  </Svg>
);

export const IconInfo = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="6" />
    <path d="M8 7.2V11M8 5h.01" />
  </Svg>
);

export const IconWarning = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 2.2 14.2 13H1.8L8 2.2Z" />
    <path d="M8 6.5v3M8 11.2h.01" />
  </Svg>
);

export const IconError = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="6" />
    <path d="M8 4.8V8.6M8 11h.01" />
  </Svg>
);

export const IconLogo = (p: IconProps) => (
  <svg width={p.size ?? 20} height={p.size ?? 20} viewBox="0 0 20 20" fill="none" aria-hidden="true">
    <circle cx="10" cy="10" r="9" stroke="var(--accent)" strokeWidth="1.4" opacity="0.35" />
    <path
      d="M10 10c0-1.2 1.6-1.6 2.4-.6 1.2 1.5-.2 3.8-2.4 3.8-2.8 0-4.3-3.1-2.8-5.6 1.9-3.2 6.9-2.9 8.4.6"
      stroke="var(--accent)"
      strokeWidth="1.6"
      strokeLinecap="round"
    />
    <circle cx="10" cy="10" r="1.3" fill="var(--accent)" />
  </svg>
);

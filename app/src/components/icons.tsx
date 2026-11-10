import type { ReactNode, SVGProps } from "react";

export type IconProps = SVGProps<SVGSVGElement>;

function Svg({ className, children, viewBox = "0 0 16 16", ...rest }: IconProps & { children: ReactNode }) {
  return (
    <svg
      className={className ? `ds-icon ${className}` : "ds-icon"}
      viewBox={viewBox}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
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

export const IconExport = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 10V2.5M5 5.5l3-3 3 3" />
    <path d="M3 9v3.5a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V9" />
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

export const IconChevronDown = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4.5 6.5 8 10l3.5-3.5" />
  </Svg>
);

export const IconCopy = (p: IconProps) => (
  <Svg {...p}>
    <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
    <path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2" />
  </Svg>
);

export const IconCheck = (p: IconProps) => (
  <Svg {...p}>
    <path d="M3 8.5 6.5 12 13 4.5" />
  </Svg>
);

export const IconTrash = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.5 4.5h11M6.25 4.5V2.75h3.5V4.5" />
    <path d="M4 4.5l.65 8.6a1 1 0 0 0 1 .9h4.7a1 1 0 0 0 1-.9L12 4.5" />
    <path d="M6.75 7v4M9.25 7v4" />
  </Svg>
);

export const IconEye = (p: IconProps) => (
  <Svg {...p}>
    <path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8Z" />
    <circle cx="8" cy="8" r="2" />
  </Svg>
);

export const IconEyeOff = (p: IconProps) => (
  <Svg {...p}>
    <path d="M6.6 3.65A6 6 0 0 1 8 3.5c4 0 6.5 4.5 6.5 4.5a11 11 0 0 1-1.7 2.2M10.6 11.9A5.6 5.6 0 0 1 8 12.5C4 12.5 1.5 8 1.5 8a11.3 11.3 0 0 1 2.6-3.1" />
    <path d="M6.6 6.6a2 2 0 0 0 2.8 2.8" />
    <path d="M2.5 2.5l11 11" />
  </Svg>
);

export const IconSettings = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.5 3.5h6M11.5 3.5h2M2.5 8h1.5M7 8h6.5M2.5 12.5h5M10.5 12.5h3" />
    <circle cx="10" cy="3.5" r="1.5" />
    <circle cx="5.5" cy="8" r="1.5" />
    <circle cx="9" cy="12.5" r="1.5" />
  </Svg>
);

export const IconSelect = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 2.5v10l2.7-2.4 1.8 3.9 1.7-.8-1.8-3.8H12L4 2.5Z" />
  </Svg>
);

export const IconPan = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 2v12M2 8h12M6.25 3.75 8 2l1.75 1.75M6.25 12.25 8 14l1.75-1.75M3.75 6.25 2 8l1.75 1.75M12.25 6.25 14 8l-1.75 1.75" />
  </Svg>
);

export const IconTrajectoryAdd = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2 13.5C3 7.5 6.5 3.5 13 2.5" />
    <path d="M11.5 8.5v5M9 11h5" />
  </Svg>
);

export const IconZoomFit = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2 5.5V2h3.5M10.5 2H14v3.5M14 10.5V14h-3.5M5.5 14H2v-3.5" />
    <rect x="5.5" y="5.5" width="5" height="5" rx="1" />
  </Svg>
);

export const IconResetView = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.5 8a5.5 5.5 0 1 0 1.7-4" />
    <path d="M2.5 2.5V5h2.5" />
  </Svg>
);

export const IconField = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2 12.5h4.5M5 11l1.5 1.5L5 14" />
    <path d="M7.25 9.75 10 7M8 7h2v2" />
    <path d="M12.5 6.5V2M11 3.5 12.5 2 14 3.5" />
  </Svg>
);

export const IconNullcline = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2 12.5C6 12.5 9 3.5 14 3.5" strokeDasharray="2 3" />
  </Svg>
);

export const IconFixedPoint = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="5.5" />
    <circle cx="8" cy="8" r="2" fill="currentColor" stroke="none" />
  </Svg>
);

export const IconTimeSeries = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2 2v12h12" />
    <path d="M4.5 8.5c.9-3 1.9-3 2.75 0s1.85 3 2.75 0 1.75-3.5 3.5-3.5" />
  </Svg>
);

export const IconCube3d = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 1.75l5.5 3v6.5L8 14.25l-5.5-3v-6.5L8 1.75Z" />
    <path d="M2.5 4.75 8 7.75l5.5-3M8 7.75v6.5" />
  </Svg>
);

export const IconPlay = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5 3.2v9.6L12.5 8 5 3.2Z" />
  </Svg>
);

export const IconPause = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5.75 3.5v9M10.25 3.5v9" />
  </Svg>
);

export const IconStepForward = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4.5 3.2v9.6L11 8 4.5 3.2Z" />
    <path d="M12.5 3.5v9" />
  </Svg>
);

export const IconSearch = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="7" cy="7" r="4.25" />
    <path d="m10.2 10.2 3.3 3.3" />
  </Svg>
);

/** Follow a bifurcation: a curve through a point. */
export const IconFollow = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.5 13.5C5 13 6.5 10.5 8 8s3-5 5.5-5.5" />
    <circle cx="8" cy="8" r="1.75" fill="currentColor" />
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
    <path d="M5.5 1.75h5l3.75 3.75v5l-3.75 3.75h-5L1.75 10.5v-5L5.5 1.75Z" />
    <path d="M8 4.8V8.6M8 11h.01" />
  </Svg>
);

export const IconLogo = ({ className, ...rest }: IconProps) => (
  <svg
    className={className ? `ds-icon brand-logo ${className}` : "ds-icon brand-logo"}
    viewBox="0 0 20 20"
    fill="none"
    stroke="var(--accent)"
    aria-hidden="true"
    focusable="false"
    {...rest}
  >
    <circle cx="10" cy="10" r="9" strokeWidth="1.4" opacity="0.35" />
    <path
      d="M10 10c0-1.2 1.6-1.6 2.4-.6 1.2 1.5-.2 3.8-2.4 3.8-2.8 0-4.3-3.1-2.8-5.6 1.9-3.2 6.9-2.9 8.4.6"
      strokeWidth="1.6"
      strokeLinecap="round"
    />
    <circle cx="10" cy="10" r="1.3" fill="var(--accent)" stroke="none" />
  </svg>
);

/** A blank page with a folded corner: New. */
export const IconNewDoc = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 1.75h5.25L12.5 5v8.25a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1Z" />
    <path d="M9 1.75V5.25h3.5" />
  </Svg>
);

/** An open folder: Open…. */
export const IconOpen = (p: IconProps) => (
  <Svg {...p}>
    <path d="M1.75 12.5V3.75a1 1 0 0 1 1-1h3l1.5 1.5h4.5a1 1 0 0 1 1 1V6.5" />
    <path d="M1.75 12.5l1.9-5.2a1 1 0 0 1 .94-.65h9.16a.75.75 0 0 1 .7 1l-1.75 4.6a1 1 0 0 1-.94.65H2.75a1 1 0 0 1-1-1Z" />
  </Svg>
);

/** A floppy disk: Save. */
export const IconSave = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2.75 1.75h8.5l3 3v8.5a1 1 0 0 1-1 1H2.75a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1Z" />
    <path d="M4.75 1.75v3.5h5.5v-3.5" />
    <rect x="4.25" y="8.75" width="7.5" height="5.5" rx="0.5" />
  </Svg>
);

/** A floppy disk with a pencil: Save as…. */
export const IconSaveAs = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8.25 14.25h-5.5a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1h8.5l3 3v2" />
    <path d="M4.75 1.75v3.5h5.5v-3.5" />
    <path d="M10 14.5l.4-1.9 3.35-3.35a1 1 0 0 1 1.5 1.5L11.9 14.1Z" />
  </Svg>
);

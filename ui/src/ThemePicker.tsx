import { type PointerEvent, useEffect, useRef, useState } from "react";
import { send } from "./bridge";
import {
  colorAt,
  harmony,
  MAX_COLORS,
  type Mode,
  paint,
  pointOf,
  systemDark,
  type Theme,
  type Wheel,
  wheelFor,
} from "./theme";

type Detail = { space: number; theme: Theme };

// The palette's pages, nine colors each.
export const PALETTES = [
  [
    "#f3efe8",
    "#ec9cbd",
    "#9a72aa",
    "#e5525a",
    "#f47c45",
    "#f3d14b",
    "#38d27f",
    "#62b4d8",
    "#5a5d84",
  ],
  [
    "#fdfbf8",
    "#fbe8f1",
    "#ead3ea",
    "#f6cdd2",
    "#fae0d3",
    "#fdf8e5",
    "#e2fbea",
    "#ddf1fb",
    "#c6c7e3",
  ],
  [
    "#4a3b58",
    "#623855",
    "#844545",
    "#a6704a",
    "#d0b46b",
    "#cfcca8",
    "#84a985",
    "#365f4b",
    "#2e4467",
  ],
  [
    "#ffffff",
    "#e6e6e6",
    "#cccccc",
    "#b3b3b3",
    "#808080",
    "#666666",
    "#333333",
    "#1a1a1a",
    "#000000",
  ],
];

// A Space's theme, edited like Arc's (show_theme_picker in app/src/shell.rs): dots on a pad, whose
// angle is a color's hue and distance its lightness, the first one leading; the system's, a light
// or a dark appearance; a palette to start from; the colors' intensity on a wavy slider; and their
// grain on a dial. Every change shows at once in the window. Escape and losing the focus close it.
export function ThemePicker() {
  const [shown, setShown] = useState<Detail>();
  const [theme, setTheme] = useState<Theme>();
  const [wheel, setWheel] = useState<Wheel>();
  const [page, setPage] = useState(0);
  // The theme to send next, at most one every frame's time while a dot or a knob moves.
  const pending = useRef<{ space: number; theme: Theme } | undefined>(undefined);
  useEffect(() => {
    const listener = (event: Event) => {
      const detail = (event as CustomEvent<Detail>).detail;
      setShown(detail);
      setTheme(detail.theme);
      setWheel(wheelFor(detail.theme));
    };
    const hide = () => send("theme/hide");
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };
    addEventListener("arbalete:theme", listener);
    addEventListener("keydown", onKey);
    addEventListener("blur", hide);
    return () => {
      removeEventListener("arbalete:theme", listener);
      removeEventListener("keydown", onKey);
      removeEventListener("blur", hide);
    };
  }, []);
  if (!shown || !theme || !wheel) return null;
  const change = (next: Partial<Theme>, nextWheel?: Wheel) => {
    const updated = { ...theme, ...next };
    setTheme(updated);
    if (nextWheel) setWheel(nextWheel);
    if (!pending.current) {
      setTimeout(() => {
        const sent = pending.current;
        pending.current = undefined;
        if (sent) {
          send(`space/theme/${sent.space}/${encodeURIComponent(JSON.stringify(sent.theme))}`);
        }
      }, 16);
    }
    pending.current = { space: shown.space, theme: updated };
  };
  const look = paint(theme, systemDark());
  const [first, second = first] = look.stops;
  const colored = theme.colors.length > 0;
  return (
    <div
      className="theme-picker"
      data-dark={look.dark || undefined}
      style={
        {
          "--stop-1": first,
          "--stop-2": second,
          colorScheme: look.dark ? "dark" : "light",
        } as React.CSSProperties
      }
    >
      <Pad theme={theme} wheel={wheel} onChange={change} />
      <ModeButtons
        mode={theme.mode}
        onMode={(mode) => change({ mode }, wheelFor({ ...theme, mode }))}
      />
      <div className="theme-palette">
        <button
          type="button"
          className="theme-button theme-page"
          aria-label="Previous colors"
          disabled={page === 0}
          onClick={() => setPage(page - 1)}
        >
          <Chevron left />
        </button>
        {PALETTES[page].map((color) => (
          <button
            key={color}
            type="button"
            className="theme-swatch"
            aria-label={color}
            style={{ background: color }}
            onClick={() => {
              const colors = harmony(color, Math.max(1, theme.colors.length));
              change({ colors }, wheelFor({ ...theme, colors }));
            }}
          />
        ))}
        <button
          type="button"
          className="theme-button theme-page"
          aria-label="More colors"
          disabled={page === PALETTES.length - 1}
          onClick={() => setPage(page + 1)}
        >
          <Chevron />
        </button>
      </div>
      <div className="theme-knobs">
        <SineSlider
          value={theme.intensity}
          enabled={colored}
          onChange={(intensity) => change({ intensity })}
        />
        <Dial value={theme.noise} enabled={colored} onChange={(noise) => change({ noise })} />
      </div>
    </div>
  );
}

// The pad's dots: a click on an empty pad picks the first color, a drag moves a dot, the leading one
// taking the others along; − and + remove the last color and add one that goes with the others.
function Pad({
  theme,
  wheel,
  onChange,
}: {
  theme: Theme;
  wheel: Wheel;
  onChange: (next: Partial<Theme>) => void;
}) {
  const pad = useRef<HTMLDivElement>(null);
  const [dragged, setDragged] = useState<number>();
  const colors = theme.colors;
  // A pointer event's point from the pad's center, its radius being 1.
  const at = (event: PointerEvent) => {
    const box = pad.current?.getBoundingClientRect();
    if (!box) return { x: 0, y: 0 };
    const radius = Math.min(box.width, box.height) / 2;
    return {
      x: (event.clientX - box.left - box.width / 2) / radius,
      y: (event.clientY - box.top - box.height / 2) / radius,
    };
  };
  const move = (index: number, x: number, y: number) => {
    const color = colorAt(wheel, x, y);
    if (index > 0) {
      onChange({ colors: colors.map((c, i) => (i === index ? color : c)) });
      return;
    }
    // The other dots turn and stretch with the leading one.
    const from = pointOf(wheel, colors[0]);
    const turn = Math.atan2(y, x) - Math.atan2(from.y, from.x);
    const stretch =
      Math.hypot(from.x, from.y) > 0.01 ? Math.hypot(x, y) / Math.hypot(from.x, from.y) : 1;
    const others = colors.slice(1).map((c) => {
      const p = pointOf(wheel, c);
      const [cos, sin] = [Math.cos(turn), Math.sin(turn)];
      return colorAt(wheel, (p.x * cos - p.y * sin) * stretch, (p.x * sin + p.y * cos) * stretch);
    });
    onChange({ colors: [color, ...others] });
  };
  return (
    <div className="theme-pad">
      <div
        className="theme-wheel"
        ref={pad}
        onPointerDown={(event) => {
          const { x, y } = at(event);
          event.currentTarget.setPointerCapture?.(event.pointerId);
          if (colors.length === 0) onChange({ colors: [colorAt(wheel, x, y)] });
          else move(0, x, y);
          setDragged(0);
        }}
        onPointerMove={(event) => {
          if (dragged === undefined) return;
          const { x, y } = at(event);
          move(dragged, x, y);
        }}
        onPointerUp={() => setDragged(undefined)}
        onPointerCancel={() => setDragged(undefined)}
      >
        {colors.length === 0 && <p className="theme-empty">Tap to pick a color for this space</p>}
        {colors.map((color, index) => {
          const { x, y } = pointOf(wheel, color);
          return (
            <span
              // biome-ignore lint/suspicious/noArrayIndexKey: a dot is its place in the theme.
              key={index}
              className="theme-dot"
              data-primary={index === 0 || undefined}
              style={{ left: `${50 + x * 50}%`, top: `${50 + y * 50}%`, background: color }}
              onPointerDown={(event) => {
                event.stopPropagation();
                pad.current?.setPointerCapture?.(event.pointerId);
                setDragged(index);
              }}
            />
          );
        })}
      </div>
      <div className="theme-edit">
        <button
          type="button"
          className="theme-button"
          aria-label="Remove a color"
          disabled={colors.length === 0}
          onClick={() => onChange({ colors: colors.slice(0, -1) })}
        >
          <svg className="theme-icon" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M3 8h10" />
          </svg>
        </button>
        <button
          type="button"
          className="theme-button"
          aria-label="Add a color"
          disabled={colors.length === MAX_COLORS}
          onClick={() => {
            const lead = colors[0] ?? colorAt(wheel, 0, -0.5);
            onChange({
              colors: [...colors, ...harmony(lead, colors.length + 1).slice(colors.length)],
            });
          }}
        >
          <svg className="theme-icon" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M3 8h10M8 3v10" />
          </svg>
        </button>
      </div>
    </div>
  );
}

const MODES: { mode: Mode; label: string; d: string }[] = [
  {
    mode: "auto",
    label: "Automatic appearance",
    d: "M7 2.5l1.1 3.4L11.5 7 8.1 8.1 7 11.5 5.9 8.1 2.5 7l3.4-1.1zM12 10l.5 1.5L14 12l-1.5.5L12 14l-.5-1.5L10 12l1.5-.5zM12.5 2.5l.4 1.1 1.1.4-1.1.4-.4 1.1-.4-1.1-1.1-.4 1.1-.4z",
  },
  {
    mode: "light",
    label: "Light appearance",
    d: "M8 5.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5zM8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1",
  },
  {
    mode: "dark",
    label: "Dark appearance",
    d: "M10.5 11.5A5 5 0 0 1 6 3a5 5 0 1 0 6.5 6.8 5 5 0 0 1-2 1.7zM12 2.5l.4 1.1 1.1.4-1.1.4-.4 1.1-.4-1.1-1.1-.4 1.1-.4z",
  },
];

function ModeButtons({ mode, onMode }: { mode: Mode; onMode: (mode: Mode) => void }) {
  return (
    <div className="theme-modes">
      {MODES.map((option) => (
        <button
          key={option.mode}
          type="button"
          className="theme-button"
          aria-label={option.label}
          aria-pressed={mode === option.mode}
          onClick={() => onMode(option.mode)}
        >
          <svg className="theme-icon" viewBox="0 0 16 16" aria-hidden="true">
            <path d={option.d} />
          </svg>
        </button>
      ))}
    </div>
  );
}

function Chevron({ left }: { left?: boolean }) {
  return (
    <svg className="theme-icon" viewBox="0 0 16 16" aria-hidden="true">
      <path d={left ? "M10 3L5 8l5 5" : "M6 3l5 5-5 5"} />
    </svg>
  );
}

// The slider's size, in points, and how many waves it shows across.
const SLIDER = { width: 228, height: 64, waves: 6, amplitude: 13 };

// The colors' intensity, from 0 to 1: a wave up to the thumb, as tall as the value, and its rest
// dimmed past it.
export function SineSlider({
  value,
  enabled,
  onChange,
}: {
  value: number;
  enabled: boolean;
  onChange: (value: number) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const { width, height, waves, amplitude } = SLIDER;
  const thumb = 14 + value * (width - 28);
  const wave = (from: number, to: number) => {
    const points = [];
    for (let x = from; x <= to; x += 2) {
      const y = height / 2 - Math.sin((x / width) * waves * 2 * Math.PI) * amplitude * value;
      points.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    }
    return `M${points.join("L")}`;
  };
  const set = (event: PointerEvent) => {
    const box = event.currentTarget.getBoundingClientRect();
    const x = ((event.clientX - box.left) / box.width) * width;
    onChange(Math.min(1, Math.max(0, (x - 14) / (width - 28))));
  };
  return (
    <div
      className="sine-slider"
      role="slider"
      tabIndex={enabled ? 0 : -1}
      aria-label="Intensity"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(value * 100)}
      aria-disabled={!enabled}
      onPointerDown={(event) => {
        if (!enabled) return;
        event.currentTarget.setPointerCapture?.(event.pointerId);
        setDragging(true);
        set(event);
      }}
      onPointerMove={(event) => dragging && set(event)}
      onPointerUp={() => setDragging(false)}
      onKeyDown={(event) => {
        const step = { ArrowRight: 0.05, ArrowUp: 0.05, ArrowLeft: -0.05, ArrowDown: -0.05 }[
          event.key
        ];
        if (enabled && step) onChange(Math.min(1, Math.max(0, value + step)));
      }}
    >
      <svg className="theme-graphic" viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
        <rect className="sine-track" x="0" y={height / 2 - 16} width={width} height="32" rx="16" />
        <path className="sine-rest" d={wave(thumb, width - 12)} />
        <path className="sine-wave" d={wave(12, thumb)} />
        <rect className="sine-thumb" x={thumb - 14} y="4" width="28" height={height - 8} rx="14" />
      </svg>
    </div>
  );
}

// The dial's dots, and the turn they span from the top, clockwise, leaving a gap before the top.
const DIAL = { dots: 24, span: 2 * Math.PI - 0.25, size: 92 };

// The grain, from 0 to 1, on a dial: the dots up to its handle light up, and the disk shows it.
export function Dial({
  value,
  enabled,
  onChange,
}: {
  value: number;
  enabled: boolean;
  onChange: (value: number) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const { dots, span, size } = DIAL;
  const center = size / 2;
  const ring = size / 2 - 6;
  const angle = (fraction: number) => fraction * span - Math.PI / 2;
  const turn = (event: PointerEvent) => {
    const box = event.currentTarget.getBoundingClientRect();
    const x = event.clientX - box.left - box.width / 2;
    const y = event.clientY - box.top - box.height / 2;
    const from = (Math.atan2(y, x) + Math.PI / 2 + 2 * Math.PI) % (2 * Math.PI);
    onChange(Math.min(1, from / span));
  };
  const handle = angle(value);
  return (
    <div
      className="theme-dial"
      role="slider"
      tabIndex={enabled ? 0 : -1}
      aria-label="Grain"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(value * 100)}
      aria-disabled={!enabled}
      onPointerDown={(event) => {
        if (!enabled) return;
        event.currentTarget.setPointerCapture?.(event.pointerId);
        setDragging(true);
        turn(event);
      }}
      onPointerMove={(event) => dragging && turn(event)}
      onPointerUp={() => setDragging(false)}
      onWheel={(event) => {
        if (enabled) onChange(Math.min(1, Math.max(0, value - event.deltaY / 500)));
      }}
      onKeyDown={(event) => {
        const step = { ArrowRight: 0.05, ArrowUp: 0.05, ArrowLeft: -0.05, ArrowDown: -0.05 }[
          event.key
        ];
        if (enabled && step) onChange(Math.min(1, Math.max(0, value + step)));
      }}
    >
      <svg className="theme-graphic" viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
        <defs>
          <filter id="dial-noise">
            <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" />
            <feColorMatrix values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 1.6 -0.5" />
            <feComposite in2="SourceGraphic" operator="in" />
          </filter>
        </defs>
        {Array.from({ length: dots }, (_, i) => {
          const fraction = i / (dots - 1);
          const a = angle(fraction);
          return (
            <circle
              // biome-ignore lint/suspicious/noArrayIndexKey: the dots never move.
              key={i}
              className="dial-dot"
              data-lit={enabled && fraction <= value + 1e-6 && value > 0 ? "" : undefined}
              cx={center + Math.cos(a) * ring}
              cy={center + Math.sin(a) * ring}
              r="2.6"
            />
          );
        })}
        <circle className="dial-disk" cx={center} cy={center} r={ring - 12} />
        <circle
          className="dial-grain"
          cx={center}
          cy={center}
          r={ring - 12}
          filter="url(#dial-noise)"
          style={{ opacity: value }}
        />
        <line
          className="dial-handle"
          x1={center + Math.cos(handle) * (ring - 6)}
          y1={center + Math.sin(handle) * (ring - 6)}
          x2={center + Math.cos(handle) * (ring + 4)}
          y2={center + Math.sin(handle) * (ring + 4)}
        />
      </svg>
    </div>
  );
}

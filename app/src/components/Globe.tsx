import { useMemo } from "react";
import { landDots } from "../lib/land";
import type { Region } from "../lib/regions";

const RAD = Math.PI / 180;
type V3 = [number, number, number];

const vec = (lat: number, lon: number): V3 => [Math.cos(lat * RAD) * Math.cos(lon * RAD), Math.cos(lat * RAD) * Math.sin(lon * RAD), Math.sin(lat * RAD)];
const toLatLon = ([x, y, z]: V3): [number, number] => [Math.asin(Math.max(-1, Math.min(1, z))) / RAD, Math.atan2(y, x) / RAD];

/** Orthographic projection around a centre; `front` is false for points on the far side. */
function projector(lat0: number, lon0: number, r: number, c: number) {
  const s0 = Math.sin(lat0 * RAD);
  const k0 = Math.cos(lat0 * RAD);
  return (lat: number, lon: number, lift = 1) => {
    const d = (lon - lon0) * RAD;
    const cl = Math.cos(lat * RAD);
    const x = cl * Math.sin(d);
    const y = k0 * Math.sin(lat * RAD) - s0 * cl * Math.cos(d);
    const front = s0 * Math.sin(lat * RAD) + k0 * cl * Math.cos(d);
    return { x: c + r * lift * x, y: c - r * lift * y, front: front > 0 || lift > 1.02 };
  };
}

/** Points along the great circle from a to b, lifted off the surface in the middle so the arc reads as a flight. */
function arc(a: Region, b: Region, steps = 64): { lat: number; lon: number; lift: number }[] {
  const p = vec(a.lat, a.lon);
  const q = vec(b.lat, b.lon);
  const dot = Math.max(-1, Math.min(1, p[0] * q[0] + p[1] * q[1] + p[2] * q[2]));
  const w = Math.acos(dot);
  const out = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const s = w < 1e-6 ? [1 - t, t] : [Math.sin((1 - t) * w) / Math.sin(w), Math.sin(t * w) / Math.sin(w)];
    const v: V3 = [s[0] * p[0] + s[1] * q[0], s[0] * p[1] + s[1] * q[1], s[0] * p[2] + s[1] * q[2]];
    const [lat, lon] = toLatLon(v);
    out.push({ lat, lon, lift: 1 + Math.min(0.35, 0.08 + w * 0.12) * Math.sin(Math.PI * t) });
  }
  return out;
}

/** A line of latitude or longitude as path segments, broken where it goes behind the globe. */
function line(points: { lat: number; lon: number }[], project: ReturnType<typeof projector>): string {
  let d = "";
  let drawing = false;
  for (const pt of points) {
    const p = project(pt.lat, pt.lon);
    if (!p.front) {
      drawing = false;
      continue;
    }
    d += `${drawing ? "L" : "M"}${p.x.toFixed(1)} ${p.y.toFixed(1)}`;
    drawing = true;
  }
  return d;
}

/** A globe in plain lines: this Mac, every exit node, and while on, the arc the traffic takes. */
export function Globe({ home, nodes, active, on }: { home: Region | undefined; nodes: { name: string; region: Region | undefined }[]; active: string | null; on: boolean }) {
  const size = 360;
  const c = size / 2;
  const r = 150;
  const target = nodes.find((n) => n.name === active)?.region;
  const { lat0, lon0 } = useMemo(() => {
    const pts = [home, target].filter(Boolean) as Region[];
    if (pts.length === 0) return { lat0: 30, lon0: 10 };
    const [lat, lon] = toLatLon(pts.map((p) => vec(p.lat, p.lon)).reduce((a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]]) as V3);
    return { lat0: Math.max(-35, Math.min(40, lat - 18)), lon0: lon };
  }, [home, target]);
  const project = useMemo(() => projector(lat0, lon0, r, c), [lat0, lon0, r, c]);

  const grid = useMemo(() => {
    const paths: string[] = [];
    for (let lon = -180; lon < 180; lon += 20) paths.push(line(Array.from({ length: 91 }, (_, i) => ({ lat: -90 + i * 2, lon })), project));
    for (let lat = -60; lat <= 60; lat += 20) paths.push(line(Array.from({ length: 181 }, (_, i) => ({ lat, lon: -180 + i * 2 })), project));
    return paths.filter(Boolean);
  }, [project]);

  const land = useMemo(() => landDots().map((d) => project(d.lat, d.lon)).filter((p) => p.front), [project]);

  const path = useMemo(() => {
    if (!home || !target) return "";
    return arc(home, target)
      .map((p, i) => {
        const q = project(p.lat, p.lon, p.lift);
        return `${i ? "L" : "M"}${q.x.toFixed(1)} ${q.y.toFixed(1)}`;
      })
      .join("");
  }, [home, target, project]);

  const dot = (reg: Region | undefined) => (reg ? project(reg.lat, reg.lon) : null);
  const h = dot(home);
  const label = home && target ? `Traffic goes from ${home.city} to ${target.city}` : home ? `This Mac near ${home.city}` : "The tailnet";

  return (
    <svg role="img" aria-label={on ? label : `VPN off. ${label.replace("Traffic goes", "Traffic would go")}`} viewBox={`0 0 ${size} ${size}`} className="h-auto w-full max-w-[420px]">
      <defs>
        <radialGradient id="globe-fill" cx="38%" cy="32%" r="75%">
          <stop offset="0" stopColor="#1c1c1f" />
          <stop offset="1" stopColor="#0b0b0c" />
        </radialGradient>
        <radialGradient id="globe-glow" cx="50%" cy="50%" r="50%">
          <stop offset="0.82" stopColor="#4cb782" stopOpacity="0" />
          <stop offset="1" stopColor="#4cb782" stopOpacity={on ? 0.12 : 0} />
        </radialGradient>
      </defs>
      <circle cx={c} cy={c} r={r + 16} fill="url(#globe-glow)" className="transition-opacity duration-700" />
      <circle cx={c} cy={c} r={r} fill="url(#globe-fill)" stroke="#34343a" />
      <g fill="none" stroke="#26262a" strokeWidth="0.8">
        {grid.map((d, i) => (
          <path key={i} d={d} />
        ))}
      </g>
      <g fill="#34343a">
        {land.map((p, i) => (
          <circle key={i} cx={p.x.toFixed(1)} cy={p.y.toFixed(1)} r="1.1" />
        ))}
      </g>
      {path && (
        <g fill="none" strokeLinecap="round" className={`transition-opacity duration-500 ${on ? "opacity-100" : "opacity-25"}`}>
          <path d={path} stroke="#4cb782" strokeOpacity="0.25" strokeWidth="3" />
          <path d={path} stroke="#4cb782" strokeWidth="1.5" strokeDasharray="4 10" className={on ? "vpn-flow" : ""} />
        </g>
      )}
      {nodes.map((n) => {
        const p = dot(n.region);
        if (!p?.front) return null;
        const live = on && n.name === active;
        return (
          <g key={n.name}>
            {live && <circle cx={p.x} cy={p.y} r="6" fill="none" stroke="#4cb782" className="vpn-pulse" style={{ transformOrigin: `${p.x}px ${p.y}px` }} />}
            <circle cx={p.x} cy={p.y} r={n.name === active ? 4 : 3} fill={live ? "#4cb782" : n.name === active ? "#ededed" : "#6b6b6b"} />
            <text x={p.x + 8} y={p.y + 3} fontSize="10" fill={n.name === active ? "#ededed" : "#6b6b6b"} fontFamily="Inter, sans-serif">
              {n.region?.city ?? n.name}
            </text>
          </g>
        );
      })}
      {h?.front && (
        <g>
          <circle cx={h.x} cy={h.y} r="4" fill="#0b0b0c" stroke="#ededed" strokeWidth="1.5" />
          <text x={h.x - 8} y={h.y - 6} textAnchor="end" fontSize="10" fill="#a1a1a1" fontFamily="Inter, sans-serif">
            This Mac
          </text>
        </g>
      )}
    </svg>
  );
}

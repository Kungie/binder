import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost";

const VARIANT: Record<Variant, string> = {
  primary: "bg-text text-bg hover:bg-white font-medium",
  secondary: "border border-line-strong text-text hover:bg-raised",
  ghost: "text-text-2 hover:bg-raised hover:text-text",
};

export function Button({ variant = "secondary", className = "", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type="button"
      className={`inline-flex h-7 items-center gap-1.5 rounded-control px-2.5 text-[12px] transition-colors duration-[120ms] disabled:opacity-50 ${VARIANT[variant]} ${className}`}
      {...rest}
    />
  );
}

export const fieldClass =
  "h-7 w-full rounded-control border border-line bg-card px-2 text-[12px] text-text placeholder:text-text-3 focus:border-text-3 focus:outline-none";

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="grid grid-cols-[88px_minmax(0,1fr)] items-center gap-2 text-[12px]">
      <span className="text-text-3">{label}</span>
      {children}
    </label>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="rounded-[4px] border border-line px-1 font-mono text-[11px] text-text-3">{children}</kbd>;
}

export function Notice({ children }: { children: ReactNode }) {
  return <p className="text-[12px] text-text-2">{children}</p>;
}

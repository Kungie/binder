import { useState } from "react";
import { derivePrefix } from "../lib/memory";
import { pickFolder } from "../lib/pick";
import { useStore } from "../lib/store";
import { Button, Field, fieldClass } from "./ui";

export function NewProject() {
  const { projects, createProject, setCreatingProject, openProject } = useStore();
  const [name, setName] = useState("");
  const [prefix, setPrefix] = useState("");
  const [repo, setRepo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const close = () => setCreatingProject(false);
  const derived = derivePrefix(name, projects.map((p) => p.prefix));

  const submit = async () => {
    if (!name.trim()) return setError("Give it a name.");
    try {
      const p = await createProject({ name, prefix: prefix || undefined, repo: repo ?? undefined });
      close();
      openProject(p.id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/60 pt-[18vh]" onMouseDown={close}>
      <div
        role="dialog"
        aria-label="New project"
        className="fade-in flex w-[460px] flex-col gap-3 rounded-panel border border-line-strong bg-panel p-4 shadow-menu"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") return close();
          if (e.key === "Enter") {
            e.preventDefault();
            void submit();
          }
        }}
      >
        <Field label="Name">
          <input autoFocus className={fieldClass} value={name} onChange={(e) => (setName(e.target.value), setError(null))} placeholder="orbit" />
        </Field>
        <Field label="Prefix">
          <input className={`${fieldClass} font-mono uppercase`} value={prefix} onChange={(e) => setPrefix(e.target.value)} placeholder={derived} maxLength={5} />
        </Field>
        <Field label="Repo folder">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate font-mono text-[11px] text-text-2">{repo ?? "None"}</span>
            <Button variant="ghost" className="ml-auto" onClick={() => void pickFolder().then((f) => f && setRepo(f))}>
              Choose
            </Button>
          </div>
        </Field>
        <div className="mt-1 flex items-center gap-2">
          {error && <span className="text-[12px] text-text-2">{error}</span>}
          <Button variant="primary" className="ml-auto" onClick={() => void submit()}>
            Create project
          </Button>
        </div>
      </div>
    </div>
  );
}

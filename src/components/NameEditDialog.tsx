import { useEffect, useRef, useState, type FormEvent } from "react";
import { X } from "lucide-react";

export function NameEditDialog(props: {
  title: string;
  description: string;
  initialName: string;
  submitLabel: string;
  maxLength?: number;
  onSave: (name: string) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(props.initialName);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !savingRef.current) props.onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const nextName = name.trim();
    if (!nextName) {
      setError("Enter a name to continue.");
      inputRef.current?.focus();
      return;
    }
    if (nextName === props.initialName.trim()) {
      props.onClose();
      return;
    }
    savingRef.current = true;
    setSaving(true);
    setError("");
    try {
      await props.onSave(nextName);
      props.onClose();
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : String(saveError));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }

  return (
    <div className="modal-backdrop name-edit-backdrop" onMouseDown={() => { if (!savingRef.current) props.onClose(); }}>
      <form
        className="modal name-edit-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="name-edit-title"
        onSubmit={submit}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-head">
          <div>
            <span className="eyebrow">Dusk</span>
            <h2 id="name-edit-title">{props.title}</h2>
            <p>{props.description}</p>
          </div>
          <button type="button" className="icon-button" onClick={props.onClose} aria-label="Close" disabled={saving}>
            <X size={17} />
          </button>
        </div>
        <label>
          <span>Name</span>
          <input
            ref={inputRef}
            value={name}
            maxLength={props.maxLength}
            autoComplete="off"
            onChange={(event) => setName(event.target.value)}
            placeholder="Enter a name"
            disabled={saving}
          />
        </label>
        {error && <div className="inline-error" role="alert">{error}</div>}
        <div className="modal-actions">
          <button type="button" className="button ghost" onClick={props.onClose} disabled={saving}>Cancel</button>
          <button className="button primary" disabled={saving || !name.trim()}>
            {saving ? "Saving…" : props.submitLabel}
          </button>
        </div>
      </form>
    </div>
  );
}

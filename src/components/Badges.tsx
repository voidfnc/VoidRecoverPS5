import { useState } from "react";
import type { Availability, PackageType, SourceState, SourceType } from "../api/types";
import { useT } from "../i18n";
import { availabilityLabel, packageTypeShort, sourceStateLabel, sourceTypeLabel } from "../lib/labels";

export function TypeBadge({ type, variant }: { type: PackageType; variant?: string | null }) {
  const t = useT();
  const short = packageTypeShort(t, type);
  return <span className={`badge type-${type}`}>{variant ? `${short} · ${variant}` : short}</span>;
}

export function AvailabilityBadge({ value }: { value: Availability }) {
  const t = useT();
  return <span className={`badge avail-${value}`}>{availabilityLabel(t, value)}</span>;
}

export function StateBadge({ state }: { state: SourceState }) {
  const t = useT();
  return <span className={`badge state-${state}`}>{sourceStateLabel(t, state)}</span>;
}

export function ProvenanceBadge({ type }: { type: SourceType }) {
  const t = useT();
  return <span className={`badge prov prov-${type}`}>{sourceTypeLabel(t, type)}</span>;
}

export function CopyButton({ text, label }: { text: string; label?: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const shown = label ?? t("common.copy");
  return (
    <button
      className="btn tiny"
      onClick={() =>
        navigator.clipboard.writeText(text).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1200);
        })
      }
      aria-label={t("common.copyAria", { label: shown, text })}
    >
      {copied ? t("common.copied") : shown}
    </button>
  );
}

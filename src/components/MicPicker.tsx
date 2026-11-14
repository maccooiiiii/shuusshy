import { type DeviceState, formatLabel, selectableMics } from "../lib/devices";

interface Props {
  state: DeviceState;
  busy: boolean;
  onSelect: (id: string) => void;
  onRefresh: () => void;
}

/** List of selectable physical microphones. Virtual devices are hidden (they would loop audio). */
export function MicPicker({ state, busy, onSelect, onRefresh }: Props) {
  const mics = selectableMics(state);
  const hidden = state.inputs.length - mics.length;
  const selectedId = state.mic.state === "disconnected" ? state.mic.id : "device" in state.mic ? state.mic.device.id : null;

  return (
    <div className="flex flex-col gap-1">
      <ul role="radiogroup" aria-label="Microphone" className="flex flex-col gap-1">
        {mics.length === 0 && (
          <li className="px-2 py-1.5 text-[12px] text-warn">No physical microphones found.</li>
        )}
        {mics.map((d) => {
          const selected = d.id === selectedId;
          return (
            <li key={d.id}>
              <button
                type="button"
                role="radio"
                aria-checked={selected}
                disabled={busy}
                onClick={() => !selected && onSelect(d.id)}
                className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left transition-colors
                  ${selected ? "bg-accent/15 text-ink" : "text-ink/90 hover:bg-panel-hi"} disabled:opacity-60`}
              >
                <span
                  className={`grid h-3.5 w-3.5 shrink-0 place-items-center rounded-full border ${selected ? "border-accent" : "border-muted/60"}`}
                >
                  {selected && <span className="h-1.5 w-1.5 rounded-full bg-accent" />}
                </span>
                <span className="flex min-w-0 flex-col">
                  <span className="truncate text-[12px]">{d.name}</span>
                  <span className={`text-[10.5px] ${d.formatError ? "text-danger" : "text-muted"}`}>
                    {d.formatError ?? formatLabel(d.format)}
                    {d.isDefault && " · Windows default"}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      <div className="flex items-center justify-between px-2 text-[10.5px] text-muted/80">
        <span>{hidden > 0 ? `${hidden} virtual input${hidden > 1 ? "s" : ""} hidden` : " "}</span>
        <button
          type="button"
          onClick={onRefresh}
          disabled={busy}
          className="rounded px-1.5 py-0.5 text-muted hover:bg-panel-hi hover:text-ink disabled:opacity-50"
        >
          Refresh
        </button>
      </div>
    </div>
  );
}

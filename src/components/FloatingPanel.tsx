import { ReactNode, useEffect, useRef, useState } from "react";
import { Minus, Plus, X } from "lucide-react";
import { cn } from "@/lib/utils";

export function FloatingPanel(props: {
  title: string;
  x: number;
  y: number;
  width?: number;
  onClose?: () => void;
  defaultCollapsed?: boolean;
  className?: string;
  children: ReactNode;
}) {
  const [pos, setPos] = useState({ x: props.x, y: props.y });
  const [collapsed, setCollapsed] = useState(!!props.defaultCollapsed);
  const [dragging, setDragging] = useState(false);
  const off = useRef({ x: 0, y: 0 });

  function onDown(e: React.MouseEvent) {
    off.current = { x: e.clientX - pos.x, y: e.clientY - pos.y };
    setDragging(true);
    e.preventDefault();
  }

  useEffect(() => {
    if (!dragging) return;
    const mm = (e: MouseEvent) => setPos({ x: Math.max(0, e.clientX - off.current.x), y: Math.max(0, e.clientY - off.current.y) });
    const mu = () => setDragging(false);
    window.addEventListener("mousemove", mm);
    window.addEventListener("mouseup", mu);
    return () => {
      window.removeEventListener("mousemove", mm);
      window.removeEventListener("mouseup", mu);
    };
  }, [dragging]);

  return (
    <div
      className={cn("glass fixed z-30 flex flex-col overflow-hidden rounded-2xl", props.className)}
      style={{ left: pos.x, top: pos.y, width: props.width ?? 280 }}
    >
      <div className="flex cursor-grab items-center justify-between px-3.5 py-2 active:cursor-grabbing select-none" onMouseDown={onDown}>
        <span className="text-[13px] font-semibold tracking-tight">{props.title}</span>
        <div className="flex items-center gap-1">
          <button className="grid h-5 w-5 place-items-center rounded-md text-muted-foreground hover:bg-black/5" onClick={() => setCollapsed((c) => !c)}>
            {collapsed ? <Plus className="h-3.5 w-3.5" /> : <Minus className="h-3.5 w-3.5" />}
          </button>
          {props.onClose && (
            <button className="grid h-5 w-5 place-items-center rounded-md text-muted-foreground hover:bg-black/5" onClick={props.onClose}>
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      </div>
      {!collapsed && <div className="max-h-[70vh] overflow-y-auto px-3.5 pb-3.5">{props.children}</div>}
    </div>
  );
}

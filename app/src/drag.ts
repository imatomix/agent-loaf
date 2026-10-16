// 行を押すと詳細を開く操作と区別するため、この距離を動かしてからドラッグとみなす
const DRAG_THRESHOLD_PX = 4;

interface DragState {
  id: string;
  row: HTMLElement;
  pointerId: number;
  startY: number;
  started: boolean;
  beforeId: string | null;
}

export function setupDrag(
  listEl: HTMLElement,
  onDrop: (id: string, beforeId: string | null) => void,
): { isDragging(): boolean; consumeClick(): boolean } {
  const indicator = document.createElement("div");
  indicator.className = "drop-indicator";
  let drag: DragState | null = null;
  let suppressClick = false;

  const rows = () => [...listEl.querySelectorAll<HTMLElement>(".row[data-id]")];

  function targetFor(clientY: number, dragged: HTMLElement): { beforeId: string | null; top: number } {
    const others = rows().filter((row) => row !== dragged);
    for (const row of others) {
      const box = row.getBoundingClientRect();
      if (clientY < box.top + box.height / 2) return { beforeId: row.dataset.id!, top: row.offsetTop };
    }
    const last = others.at(-1);
    return { beforeId: null, top: last ? last.offsetTop + last.offsetHeight : 0 };
  }

  listEl.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    // 詳細のボタンを押したときはドラッグを始めない
    if (target.closest(".details-toggle")) return;
    const row = target.closest<HTMLElement>(".row[data-id]");
    if (!row || event.button !== 0) return;
    drag = { id: row.dataset.id!, row, pointerId: event.pointerId, startY: event.clientY, started: false, beforeId: null };
  });

  listEl.addEventListener("pointermove", (event) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    const dy = event.clientY - drag.startY;
    if (!drag.started) {
      if (Math.abs(dy) < DRAG_THRESHOLD_PX) return;
      drag.started = true;
      // 押してから動かし始めるまでの間に一覧が描き直されていると、押した行はもう画面にないので探し直す
      const fresh = listEl.querySelector<HTMLElement>(`.row[data-id="${CSS.escape(drag.id)}"]`);
      if (!fresh) {
        drag = null;
        return;
      }
      drag.row = fresh;
      listEl.setPointerCapture(drag.pointerId);
      drag.row.classList.add("dragging");
      listEl.append(indicator);
    }
    drag.row.style.transform = `translateY(${dy}px)`;
    const target = targetFor(event.clientY, drag.row);
    drag.beforeId = target.beforeId;
    indicator.style.top = `${target.top}px`;
  });

  function finish(drop: boolean): void {
    if (!drag) return;
    if (drag.started) {
      // 離した直後のクリックだけを打ち消す。行は描き直されてクリックが届かないこともあるので、すぐに解除する
      suppressClick = true;
      setTimeout(() => {
        suppressClick = false;
      }, 0);
      drag.row.classList.remove("dragging");
      drag.row.style.transform = "";
      indicator.remove();
      if (drop) onDrop(drag.id, drag.beforeId);
    }
    drag = null;
  }
  listEl.addEventListener("pointerup", () => finish(true));
  listEl.addEventListener("pointercancel", () => finish(false));

  return {
    isDragging: () => drag?.started ?? false,
    consumeClick: () => {
      const suppressed = suppressClick;
      suppressClick = false;
      return suppressed;
    },
  };
}

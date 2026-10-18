// 行を押すと詳細を開く操作と区別するため、この距離を動かしてからドラッグとみなす
const DRAG_THRESHOLD_PX = 4;

interface Slot {
  id: string;
  row: HTMLElement;
  middle: number;
}

interface DragState {
  id: string;
  row: HTMLElement;
  pointerId: number;
  startY: number;
  started: boolean;
  beforeId: string | null;
  // ほかの行は、すきまを空けるためにずれる。ずれた後の位置で判定すると行き来するので、始めたときの位置で決める
  others: Slot[];
  draggedIndex: number;
  gap: number;
}

export function setupDrag(
  listEl: HTMLElement,
  onDrop: (id: string, beforeId: string | null) => void,
): { isDragging(): boolean; consumeClick(): boolean } {
  let drag: DragState | null = null;
  let suppressClick = false;

  const rows = () => [...listEl.querySelectorAll<HTMLElement>(".row[data-id]")];

  // 一覧は中でスクロールするので、行の offsetTop と同じ一覧の中の座標にそろえる
  const pointerInList = (clientY: number) => clientY - listEl.getBoundingClientRect().top + listEl.scrollTop;

  function begin(state: DragState): boolean {
    const all = rows();
    const draggedIndex = all.indexOf(state.row);
    if (draggedIndex < 0) return false;
    state.draggedIndex = draggedIndex;
    state.gap = state.row.offsetHeight;
    state.others = all
      .filter((row) => row !== state.row)
      .map((row) => ({ id: row.dataset.id!, row, middle: row.offsetTop + row.offsetHeight / 2 }));
    return true;
  }

  function moveOthers(state: DragState, clientY: number): void {
    const y = pointerInList(clientY);
    const insertAt = state.others.filter((slot) => slot.middle < y).length;
    state.beforeId = state.others[insertAt]?.id ?? null;
    state.others.forEach((slot, index) => {
      // 握った行より上にあった行は index のまま、下にあった行は 1 つ詰めて数えている
      const wasAbove = index < state.draggedIndex;
      const shift = wasAbove && index >= insertAt ? state.gap : !wasAbove && index < insertAt ? -state.gap : 0;
      slot.row.style.transform = shift === 0 ? "" : `translateY(${shift}px)`;
    });
  }

  listEl.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    // 詳細のボタンを押したときはドラッグを始めない
    if (target.closest(".details-toggle")) return;
    const row = target.closest<HTMLElement>(".row[data-id]");
    if (!row || event.button !== 0) return;
    drag = {
      id: row.dataset.id!,
      row,
      pointerId: event.pointerId,
      startY: event.clientY,
      started: false,
      beforeId: null,
      others: [],
      draggedIndex: 0,
      gap: 0,
    };
  });

  listEl.addEventListener("pointermove", (event) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    const dy = event.clientY - drag.startY;
    if (!drag.started) {
      if (Math.abs(dy) < DRAG_THRESHOLD_PX) return;
      // 押してから動かし始めるまでの間に一覧が描き直されていると、押した行はもう画面にないので探し直す
      const fresh = listEl.querySelector<HTMLElement>(`.row[data-id="${CSS.escape(drag.id)}"]`);
      if (!fresh) {
        drag = null;
        return;
      }
      drag.row = fresh;
      if (!begin(drag)) {
        drag = null;
        return;
      }
      drag.started = true;
      listEl.setPointerCapture(drag.pointerId);
      listEl.classList.add("reordering");
      drag.row.classList.add("dragging");
    }
    drag.row.style.transform = `translateY(${dy}px)`;
    moveOthers(drag, event.clientY);
  });

  function finish(drop: boolean): void {
    const ended = drag;
    if (!ended) return;
    // ドラッグ中は一覧を描き直さないので、onDrop で新しい並びを描けるよう、先に終わったことにする
    drag = null;
    if (!ended.started) return;
    // 離した直後のクリックだけを打ち消す。行は描き直されてクリックが届かないこともあるので、すぐに解除する
    suppressClick = true;
    setTimeout(() => {
      suppressClick = false;
    }, 0);
    listEl.classList.remove("reordering");
    ended.row.classList.remove("dragging");
    for (const row of rows()) row.style.transform = "";
    if (drop) onDrop(ended.id, ended.beforeId);
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

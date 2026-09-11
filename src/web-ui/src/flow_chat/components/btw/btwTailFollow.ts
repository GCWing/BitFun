/** Bind user intent separately from scroll geometry and programmatic writes. */
export function bindBtwTailFollow(
  scroller: HTMLElement,
  setFollowing: (following: boolean) => void,
  updateAffordance: () => void,
) {
  let direction = 0;
  let previousTop = scroller.scrollTop;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let draggingScrollbar = false;
  let touchY: number | undefined;
  const clearIntent = () => {
    direction = 0;
    clearTimeout(timer);
  };
  // Fallback for hosts without scrollend; renew while the gesture is moving.
  const expireIntent = () => {
    clearTimeout(timer);
    timer = setTimeout(clearIntent, 180);
  };
  const intend = (next: number) => {
    direction = next;
    previousTop = scroller.scrollTop;
    if (next < 0) setFollowing(false);
    expireIntent();
  };
  const wheel = (event: WheelEvent) => {
    if (!event.ctrlKey && event.deltaY !== 0) intend(Math.sign(event.deltaY));
  };
  const scroll = () => {
    const top = scroller.scrollTop;
    const movement = top - previousTop;
    if (draggingScrollbar && movement !== 0) {
      direction = Math.sign(movement);
      if (direction < 0) setFollowing(false);
    }
    if (direction > 0 && movement > 0 &&
      scroller.scrollHeight - top - scroller.clientHeight <= 2) {
      setFollowing(true);
      clearIntent();
    } else if (direction !== 0) {
      expireIntent();
    }
    previousTop = top;
    updateAffordance();
  };
  const keydown = (event: KeyboardEvent) => {
    // Nested controls own their keyboard behavior.
    if (event.target !== scroller || event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
    if (['ArrowUp', 'PageUp', 'Home'].includes(event.key) || (event.key === ' ' && event.shiftKey)) intend(-1);
    else if (['ArrowDown', 'PageDown', 'End', ' '].includes(event.key)) intend(1);
  };
  const touchstart = (event: TouchEvent) => {
    clearIntent();
    touchY = event.touches[0]?.clientY;
  };
  const touchmove = (event: TouchEvent) => {
    const nextY = event.touches[0]?.clientY;
    if (nextY !== undefined && touchY !== undefined && nextY !== touchY) intend(Math.sign(touchY - nextY));
    touchY = nextY;
  };
  const touchend = () => { touchY = undefined; expireIntent(); };
  const pointerdown = (event: PointerEvent) => {
    if (event.target !== scroller || event.button !== 0) return;
    const bounds = scroller.getBoundingClientRect();
    const contentLeft = bounds.left + scroller.clientLeft;
    // Only the native vertical scrollbar gutter, not clicks in the transcript.
    draggingScrollbar = event.clientX < contentLeft || event.clientX >= contentLeft + scroller.clientWidth;
    if (draggingScrollbar) {
      clearIntent();
      previousTop = scroller.scrollTop;
      setFollowing(false);
    }
  };
  const pointerup = () => { draggingScrollbar = false; clearIntent(); };
  scroller.addEventListener('wheel', wheel, { passive: true });
  scroller.addEventListener('scroll', scroll, { passive: true });
  scroller.addEventListener('scrollend', clearIntent);
  scroller.addEventListener('keydown', keydown);
  scroller.addEventListener('touchstart', touchstart, { passive: true });
  scroller.addEventListener('touchmove', touchmove, { passive: true });
  scroller.addEventListener('touchend', touchend);
  scroller.addEventListener('touchcancel', touchend);
  scroller.addEventListener('pointerdown', pointerdown);
  scroller.ownerDocument.addEventListener('pointerup', pointerup);
  scroller.ownerDocument.addEventListener('pointercancel', pointerup);
  updateAffordance();
  return () => {
    clearIntent();
    scroller.removeEventListener('wheel', wheel);
    scroller.removeEventListener('scroll', scroll);
    scroller.removeEventListener('scrollend', clearIntent);
    scroller.removeEventListener('keydown', keydown);
    scroller.removeEventListener('touchstart', touchstart);
    scroller.removeEventListener('touchmove', touchmove);
    scroller.removeEventListener('touchend', touchend);
    scroller.removeEventListener('touchcancel', touchend);
    scroller.removeEventListener('pointerdown', pointerdown);
    scroller.ownerDocument.removeEventListener('pointerup', pointerup);
    scroller.ownerDocument.removeEventListener('pointercancel', pointerup);
  };
}

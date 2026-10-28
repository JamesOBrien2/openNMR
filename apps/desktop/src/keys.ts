export const isMac = /Mac/.test(navigator.userAgent);

/** Whether a key event target is a text field or control that needs the key itself. */
export const isEditable = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable || ['INPUT', 'SELECT', 'TEXTAREA'].includes(target.tagName));

/** Whether a window is open over the app: its keys are its own then, or Z would
 *  zoom the plot behind Settings and Escape, taken as the pointer, not close it. */
export const windowOpen = () => {
  try {
    return document.querySelector('dialog:modal') !== null;
  } catch {
    // An older WebKitGTK without :modal would throw on every key.
    return false;
  }
};

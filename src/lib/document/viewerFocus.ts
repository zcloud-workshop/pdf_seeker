/** Single claimant for viewer-level global shortcuts and shared reading-state
 * writes (R29). Compare mounts two scrollers with active=true; only the panel
 * the user last interacted with may listen to window keydown and write the
 * shared zoom/page stores. Pure logic — tested under node. */

type FocusListener = () => void;

const listeners = new Set<FocusListener>();
let owner: string | null = null;

function notify(): void {
  for (const l of [...listeners]) l();
}

/** id claims global viewer focus (e.g. a scroller was activated or clicked). */
export function claimViewerFocus(id: string): void {
  if (owner !== id) {
    owner = id;
    notify();
  }
}

export function releaseViewerFocus(id: string): void {
  if (owner === id) {
    owner = null;
    notify();
  }
}

export function viewerFocusOwner(): string | null {
  return owner;
}

/** Unclaimed means anyone may act; once claimed, only the owner may. */
export function isViewerFocusOwner(id: string): boolean {
  return owner === null || owner === id;
}

export function subscribeViewerFocus(listener: FocusListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

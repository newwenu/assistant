type Listener = () => void;

let revision = 0;
const listeners = new Set<Listener>();

/** 声明被改写后通知渲染层重新读取：`src` 没变，无法靠依赖变化触发。 */
export function bumpRiveManifestRevision(): void {
  revision += 1;
  for (const listener of listeners) listener();
}

export function subscribeRiveManifestRevision(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getRiveManifestRevision(): number {
  return revision;
}

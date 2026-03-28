export const workers = Array.from(
  { length: 2 },
  () => new Worker(new URL("./worker.ts", import.meta.url)),
);

export function getWorker(lobbyId: string) {
  let hash = 0;
  for (let i = 0; i < lobbyId.length; i++) {
    hash += lobbyId.charCodeAt(i);
  }
  return workers[hash % workers.length];
}

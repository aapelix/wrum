export type WebSocketData = {
  createdAt: number;
  user: {
    id: string;
    name: string;
  };
  lobbyId?: string;
};

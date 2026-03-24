export const carTypes = ["red"] as const;
export type CarType = (typeof carTypes)[number];

type JoinData = { carType: CarType };
type LeaveData = { userId: string };
type UpdateData = { userId: string; x: number; y: number };

export type Message =
  | { type: "join"; data: JoinData }
  | { type: "leave"; data: LeaveData }
  | { type: "update"; data: UpdateData };

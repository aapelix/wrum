export type Player = {
  id: string;
  input: {
    throttle: number;
    steering: number;
  };
  x: number;
  y: number;
  rotation: number;
  carType: string;
};

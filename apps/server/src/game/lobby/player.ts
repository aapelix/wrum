import type { CarType, InputSchema } from "@wrum/shared";
import type { Collider } from "../physics/collider";

export type Player = {
  id: string;
  input: InputSchema;
  x: number;
  y: number;
  rotation: number;
  carType: CarType;

  velocity: number;
  acceleration: number;
  maxVelocity: number;
  friction: number;

  turnSpeed: number;

  tireRotation: number;
  tireTurnSpeed: number;
  tireReturnSpeed: number;
  tireMaxRotation: number;

  collider: Collider;
};

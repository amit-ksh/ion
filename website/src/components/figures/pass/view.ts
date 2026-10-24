import { boxCorners, fitProjection, iso, type Flat, type Plan, type Point3 } from '@/components/iso/iso-kit'
import { HOME_X } from './model'

export const WIDTH = 640
export const HEIGHT = 340
export const AZIMUTH = 58

export const COUNTER: Plan = { x: -186, y: -44, w: 376, d: 96, r: 8 }
export const COUNTER_H = 9
export const FOOT_H = 6

export const PRINTER: Plan = { x: -180, y: -36, w: 40, d: 52, r: 3 }
export const PRINTER_H = 92
export const FACE_X = PRINTER.x + PRINTER.w

export const RAIL_Z = 76
export const RAIL_H = 6
export const RAIL_TOP = RAIL_Z + RAIL_H
export const POST: Plan = { x: 172, y: -5, w: 10, d: 10, r: 2 }
export const RAIL: Plan = { x: FACE_X, y: -3, w: POST.x + POST.w - FACE_X, d: 6, r: 1.2 }

// A ticket is built once at HOME_X (inside the printer) and moved along x and z.
export const PAPER_Y = 4.6
export const PAPER_D = 0.8
export const PAPER_W = 22
export const PAPER_Z = 46
export const PAPER_TOP = 74
export const RIDER: Plan = { x: HOME_X - 6, y: -4.4, w: 12, d: 8.8, r: 1.6 }
export const RIDER_Z = 74.6
export const RIDER_H = 9.8

export const STATION_Y = 18
export const STATION_D = 24
export const STATION_H = 20

export const HOPPER: Plan = { x: 86, y: -30, w: 28, d: 42, r: 2 }
export const HOPPER_H = 18
export const TRAY: Plan = { x: 130, y: -12, w: 30, d: 24, r: 2 }
export const TRAY_H = 14
export const WALL = 1.6

export const DUCT: Plan = { x: FACE_X, y: -30, w: HOPPER.x - FACE_X, d: 12, r: 1.5 }
export const DUCT_H = 9

export const ANVIL: Plan = { x: 78, y: 1.4, w: 44, d: 2.8, r: 0.6 }
export const FRAME_POSTS: Plan[] = [
  { x: 78, y: -14, w: 6, d: 6, r: 1.2 },
  { x: 116, y: -14, w: 6, d: 6, r: 1.2 },
]

export const STAMP_POST: Flat = [105, 34]
export const GAUGE: Flat = [164, 32]
export const GAUGE_R = 13
export const GAUGE_TOP = 9

export const P = fitProjection(
  [
    ...boxCorners(COUNTER, -COUNTER_H - FOOT_H, 0),
    ...boxCorners(PRINTER, 0, PRINTER_H + 3),
    ...boxCorners(POST, 0, RAIL_TOP + 4),
  ],
  WIDTH,
  HEIGHT,
  { pad: 30, azimuth: AZIMUTH, headroom: 6 },
)

export const at = (point: Point3) => iso(point, P)

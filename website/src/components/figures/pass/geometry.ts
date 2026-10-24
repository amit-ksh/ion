import {
  corners,
  cylinder,
  haloOf,
  insetPlan,
  knurl,
  lineOnTop,
  onTop,
  pathOf,
  planOutline,
  radialTicks,
  ring,
  segment,
  sideArc,
  sideMatrix,
  sideRing,
  sideSeam,
  sideTicks,
  slabOf,
  topMatrix,
  depthOf,
  type Flat,
  type Plan,
  type Plane,
  type Point3,
} from '@/components/iso/iso-kit'
import { HOME_X, LIMITS, STATIONS, STROKE, TICKETS } from './model'
import {
  ANVIL,
  COUNTER,
  COUNTER_H,
  DUCT,
  DUCT_H,
  FACE_X,
  FOOT_H,
  FRAME_POSTS,
  GAUGE,
  GAUGE_R,
  GAUGE_TOP,
  HEIGHT,
  HOPPER,
  HOPPER_H,
  P,
  PAPER_D,
  PAPER_TOP,
  PAPER_W,
  PAPER_Y,
  PAPER_Z,
  POST,
  PRINTER,
  PRINTER_H,
  RAIL,
  RAIL_H,
  RAIL_TOP,
  RAIL_Z,
  RIDER,
  RIDER_H,
  RIDER_Z,
  STAMP_POST,
  STATION_D,
  STATION_H,
  STATION_Y,
  TRAY,
  TRAY_H,
  WALL,
  WIDTH,
  at,
} from './view'

const slab = (plan: Plan, z: number, h: number, steps = 4, bevel = 0) => slabOf(plan, z, h, P, steps, bevel)
const cyl = (cx: number, cy: number, r: number, z: number, h: number, steps = 28, bevel = 0) => cylinder(cx, cy, r, z, h, P, steps, bevel)
const seg = (a: Point3, b: Point3) => segment(a, b, P)
const join = (paths: string[]) => paths.filter(Boolean).join('')
const byDepth = (points: Plane[]) => [...points].sort((a, b) => depthOf([a[0], a[1], 0], P) - depthOf([b[0], b[1], 0], P))

/** A rounded rectangle in local (u, v), corner arcs sampled every 30 degrees. */
function roundRect(u0: number, u1: number, v0: number, v1: number, r: number): Plane[] {
  const out: Plane[] = []
  const arcs: Array<[number, number, number]> = [
    [u1 - r, v1 - r, 0],
    [u0 + r, v1 - r, 90],
    [u0 + r, v0 + r, 180],
    [u1 - r, v0 + r, 270],
  ]
  for (const [cu, cv, start] of arcs) {
    for (let step = 0; step <= 3; step++) {
      const angle = ((start + step * 30) * Math.PI) / 180
      out.push([cu + r * Math.cos(angle), cv + r * Math.sin(angle)])
    }
  }
  return out
}
/** Points (x, z) on the vertical face at y. */
const onFaceY = (y: number, points: Plane[], closed = true) => pathOf(points.map(([x, z]) => at([x, y, z])), closed)
/** Points (y, z) on the vertical face at x. */
const onFaceX = (x: number, points: Plane[], closed = true) => pathOf(points.map(([y, z]) => at([x, y, z])), closed)
const crossBand = (x: number, plan: Plan, z0: number, z1: number) => pathOf([at([x, plan.y, z1]), at([x, plan.y + plan.d, z1]), at([x, plan.y + plan.d, z0])])

// Counter on four feet.
const FEET = byDepth(corners(COUNTER, 20))
const COUNTER_SCREWS = corners(COUNTER, 9)
const LABEL: Plan = { x: -178, y: 26, w: 30, d: 14, r: 1.5 }
const LABEL_TOP = 0.8

export const COUNTER_ART = {
  halo: haloOf(COUNTER, -COUNTER_H - FOOT_H, COUNTER_H + FOOT_H, P),
  feet: FEET.map(([x, y]) => [cyl(x, y, 7, -COUNTER_H - FOOT_H, 1.6), cyl(x, y, 5.2, -COUNTER_H - FOOT_H + 1.6, FOOT_H - 1.6)]),
  slab: slab(COUNTER, -COUNTER_H, COUNTER_H, 8, 1.6),
  band: sideSeam(COUNTER, -COUNTER_H + 3, P, 8),
  groove: planOutline(insetPlan(COUNTER, 4.5), 0, P),
  screwRings: join(COUNTER_SCREWS.map(([x, y]) => ring(x, y, 2, 0, P, 16))),
  screws: COUNTER_SCREWS.map(([x, y]) => at([x, y, 0])),
  label: slab(LABEL, 0, LABEL_TOP, 3),
  labelLine: planOutline(insetPlan(LABEL, 1.4), LABEL_TOP, P, 3),
  labelRules: join([0, 1, 2].map((row) => onTop([[LABEL.x + 6, LABEL.y + 4 + row * 2.6], [LABEL.x + (row === 1 ? 17 : 24), LABEL.y + 4 + row * 2.6]], LABEL_TOP, P))),
  labelScrews: [at([LABEL.x + 3, LABEL.y + LABEL.d / 2, LABEL_TOP]), at([LABEL.x + LABEL.w - 3, LABEL.y + LABEL.d / 2, LABEL_TOP])],
}

// The ticket printer: the head of the line, where every assignment is issued.
const PRINTER_FRONT = PRINTER.y + PRINTER.d
const PLATE: Plan = { x: -174, y: PRINTER_FRONT, w: 26, d: 0.8, r: 0.3 }
const LID: Plan = { x: -176, y: -30, w: 32, d: 18, r: 2 }
const CAP_TOP = PRINTER_H + 3
export const PRINTER_LAMP: Plane = [-152, -6]

export const PRINTER_ART = {
  body: slab(PRINTER, 0, PRINTER_H, 4),
  seams: join([sideSeam(PRINTER, 7, P, 4), sideSeam(PRINTER, PRINTER_H - 16, P, 4)]),
  cap: slab(insetPlan(PRINTER, -1), PRINTER_H, 3, 4, 0.8),
  lid: planOutline(LID, CAP_TOP, P, 3),
  hinges: [at([LID.x + 6, LID.y, CAP_TOP]), at([LID.x + LID.w - 6, LID.y, CAP_TOP])],
  mouth: onFaceX(FACE_X, roundRect(-5.6, 11.8, 44.6, 85.4, 1.6)),
  mouthLip: onFaceX(FACE_X, roundRect(-7.2, 13.4, 43, 87, 2.6)),
  plate: slab(PLATE, 28, 14, 3),
  plateRules: join([0, 1, 2].map((row) => onFaceY(PLATE.y + PLATE.d, [[PLATE.x + 4, 38 - row * 3], [PLATE.x + (row === 2 ? 14 : 21), 38 - row * 3]], false))),
  plateScrews: [at([PLATE.x + 2, PLATE.y + PLATE.d, 30]), at([PLATE.x + PLATE.w - 2, PLATE.y + PLATE.d, 30])],
  vents: join([0, 1, 2].map((row) => onFaceY(PRINTER_FRONT, roundRect(-172, -150, 56 + row * 6, 58.6 + row * 6, 1.3)))),
  button: cyl(-170, -6, 3, CAP_TOP, 2.2),
  buttonRing: ring(-170, -6, 1.6, CAP_TOP + 2.2, P, 16),
  lampBase: cyl(PRINTER_LAMP[0], PRINTER_LAMP[1], 3.4, CAP_TOP, 1.4),
  lamp: cyl(PRINTER_LAMP[0], PRINTER_LAMP[1], 2.5, CAP_TOP + 1.4, 2.4),
}

// The return duct: a rejected ticket goes back to the head of the line.
const DUCT_TOP = DUCT_H
const HATCH: Plan = { x: -96, y: DUCT.y + 2, w: 30, d: 8, r: 1 }
export const DUCT_ART = {
  body: slab(DUCT, 0, DUCT_H, 3, 0.6),
  bands: join([-115, -45, 25].map((x) => crossBand(x, DUCT, 0, DUCT_TOP))),
  hatch: slab(HATCH, DUCT_TOP, 0.8, 3),
  hatchScrews: corners(HATCH, 2).map(([x, y]) => at([x, y, DUCT_TOP + 0.8])),
}

/** An open box split for painter's order: what lies behind a falling ticket, and what lies in front. */
function box(plan: Plan, h: number) {
  const { x, y, w, d } = plan
  const inner = { x: x + WALL, w: w - WALL * 2 }
  return {
    floor: slab(plan, 0, 1.2, 4),
    back: slab({ x: inner.x, y, w: inner.w, d: WALL, r: 0 }, 0, h, 1),
    left: slab({ x, y, w: WALL, d, r: 0.6 }, 0, h, 2, 0.3),
    right: slab({ x: x + w - WALL, y, w: WALL, d, r: 0.6 }, 0, h, 2, 0.3),
    front: slab({ x: inner.x, y: y + d - WALL, w: inner.w, d: WALL, r: 0 }, 0, h, 1, 0.3),
    rivets: [0.25, 0.5, 0.75].map((share) => at([x + w * share, y + d, h - 2.4])),
  }
}
export const HOPPER_ART = box(HOPPER, HOPPER_H)
export const TRAY_ART = box(TRAY, TRAY_H)

// The pass frame: two posts and a backing board the stamp presses against.
const ANVIL_FACE = ANVIL.y + ANVIL.d
const ANVIL_Z = 46
const ANVIL_H = 26
export const FRAME_ART = {
  feet: FRAME_POSTS.map((plan) => slab(insetPlan(plan, -2.4), 0, 1.4, 3)),
  footScrews: FRAME_POSTS.flatMap((plan) => [at([plan.x - 1, plan.y + plan.d / 2, 1.4]), at([plan.x + plan.w + 1, plan.y + plan.d / 2, 1.4])]),
  posts: FRAME_POSTS.map((plan) => slab(plan, 1.4, 64.6, 3)),
  brackets: FRAME_POSTS.map((plan) => slab({ x: plan.x, y: plan.y + plan.d, w: plan.w, d: ANVIL.y - plan.y - plan.d, r: 0.4 }, 60, 5, 2, 0.3)),
  anvil: slab(ANVIL, ANVIL_Z, ANVIL_H, 3, 0.4),
  anvilInset: onFaceY(ANVIL_FACE, roundRect(ANVIL.x + 3, ANVIL.x + ANVIL.w - 3, ANVIL_Z + 3, ANVIL_Z + ANVIL_H - 3, 1.4)),
  anvilScrews: FRAME_POSTS.map((plan) => at([plan.x + plan.w / 2, ANVIL_FACE, 62.5])),
}

// The rail on a post, with a scale every 10 units: a station every 50, the pass at 100.
const RAIL_FACE = RAIL.y + RAIL.d
const RULE = sideTicks(-130, 170, 10, 5, RAIL_FACE, RAIL_TOP, [1.6, 3.2], P)
const STOP: Plan = { x: 164, y: -4, w: 4, d: 8, r: 1 }
export const RAIL_ART = {
  postFoot: slab(insetPlan(POST, -4), 0, 2, 3),
  postScrews: corners(insetPlan(POST, -4), 2).map(([x, y]) => at([x, y, 2])),
  post: slab(POST, 2, RAIL_Z - 2, 3),
  postSeam: sideSeam(POST, 12, P, 3),
  collar: slab(insetPlan(POST, -1.2), RAIL_Z - 10, 4, 3, 0.4),
  rail: slab(RAIL, RAIL_Z, RAIL_H, 3, 0.5),
  ruleMinor: RULE.minor,
  ruleMajor: RULE.major,
  gates: join([94, 106, 139, 151].map((x) => crossBand(x, RAIL, RAIL_Z, RAIL_TOP))),
  gatePins: [100, 145].map((x) => at([x, RAIL.y + 0.8, RAIL_TOP])),
  stop: slab(STOP, RAIL_TOP, 4, 2, 0.3),
  stopScrew: at([STOP.x + STOP.w / 2, STOP.y + STOP.d / 2, RAIL_TOP + 4]),
}

// One station per agent: the lamp is lit while that agent is running.
export const STATION_ART = STATIONS.map((cx) => {
  const body: Plan = { x: cx - 17, y: STATION_Y, w: 34, d: STATION_D, r: 3 }
  const top: Plan = { x: cx - 18, y: STATION_Y - 1, w: 36, d: STATION_D + 2, r: 3.5 }
  const plate: Plan = { x: cx - 11, y: STATION_Y + STATION_D, w: 22, d: 0.8, r: 0.3 }
  const lid = STATION_H + 2.5
  const plateFace = plate.y + plate.d
  const knob: Plane = [cx - 10, STATION_Y + 11]
  const lamp: Plane = [cx + 10, STATION_Y + 11]
  return {
    body: slab(body, 0, STATION_H, 4),
    seam: sideSeam(body, 4, P, 4),
    top: slab(top, STATION_H, 2.5, 4, 0.6),
    slot: planOutline({ x: cx - 6, y: STATION_Y + 4, w: 12, d: 2.6, r: 1.2 }, lid, P, 3),
    knob: cyl(knob[0], knob[1], 3, lid, 3.4, 24),
    knurl: knurl(knob[0], knob[1], 3, lid, lid + 3.4, 22, P, 0.2),
    pointer: lineOnTop(knob, [knob[0] + 1.6, knob[1] + 2.4], lid + 3.4, P),
    lampBase: cyl(lamp[0], lamp[1], 3.4, lid, 1.4, 24),
    lamp: cyl(lamp[0], lamp[1], 2.5, lid + 1.4, 2.6, 24),
    plate: slab(plate, 6, 8, 3),
    plateRules: join([0, 1].map((row) => onFaceY(plateFace, [[plate.x + 3.5, 11.2 - row * 2.8], [plate.x + (row ? 12 : 17), 11.2 - row * 2.8]], false))),
    plateScrews: [at([plate.x + 1.8, plateFace, 10]), at([plate.x + plate.w - 1.8, plateFace, 10])],
  }
})

// The stamp: a plunger that presses the ticket under review against the board.
const [SX, SY] = STAMP_POST
// Low profile, so it only covers the bottom band of the ticket it stamps.
const HOUSING: Plan = { x: SX - 6, y: 11.6, w: 12, d: 7, r: 1.4 }
const HOUSING_Z = 45.4
const ARM_Z = 52
const PAD: Plan = { x: SX - 7, y: PAPER_Y + PAPER_D + STROKE, w: 13, d: 2.2, r: 0.5 }
const ROD: Plan = { x: SX - 1.6, y: PAD.y + PAD.d, w: 3.2, d: HOUSING.y + 3 - PAD.y - PAD.d, r: 0.8 }
export const STAMP_ART = {
  foot: cyl(SX, SY, 6, 0, 1.6, 32),
  footScrews: [0, 1, 2, 3].map((index) => {
    const angle = Math.PI / 4 + (index * Math.PI) / 2
    return at([SX + Math.cos(angle) * 4.6, SY + Math.sin(angle) * 4.6, 1.6])
  }),
  post: cyl(SX, SY, 2.6, 1.6, ARM_Z - 1.6, 24),
  postSeam: sideArc(SX, SY, 2.6, 12, P, 16),
  collar: cyl(SX, SY, 3.8, ARM_Z - 12, 3.4, 24, 0.3),
  arm: slab({ x: SX - 3.5, y: 13, w: 7, d: SY + 3.5 - 13, r: 1.2 }, ARM_Z, 3.5, 3, 0.4),
  armScrew: at([SX, SY, ARM_Z + 3.5]),
  housing: slab(HOUSING, HOUSING_Z, ARM_Z - HOUSING_Z, 3, 0.4),
  housingSeam: sideSeam(HOUSING, HOUSING_Z + 2, P, 3),
  rod: slab(ROD, 48.4, 3, 3),
  pad: slab(PAD, 46.6, 6.8, 2, 0.3),
}

// The token gauge: the needle reads the run's spend against its budget.
const [GX, GY] = GAUGE
const DIAL_FROM = (-135 * Math.PI) / 180
const DIAL_SPAN = (270 * Math.PI) / 180
const DIAL = radialTicks(GX, GY, GAUGE_R - 2.6, 40, 5, GAUGE_TOP, [1.2, 2.6], P, DIAL_FROM, DIAL_SPAN)
const dialPoint = (share: number, radius: number): Plane => {
  const angle = DIAL_FROM + share * DIAL_SPAN
  return [GX + Math.sin(angle) * radius, GY - Math.cos(angle) * radius]
}
export const GAUGE_ART = {
  flange: cyl(GX, GY, GAUGE_R + 1.6, 0, 1.4, 48),
  flangeScrews: [0.15, 0.4, 0.62].map((share) => {
    const angle = Math.PI * (0.1 + share)
    return at([GX + Math.cos(angle) * (GAUGE_R + 0.8), GY + Math.sin(angle) * (GAUGE_R + 0.8), 1.4])
  }),
  puck: cyl(GX, GY, GAUGE_R, 1.4, GAUGE_TOP - 1.4, 64, 0.6),
  band: sideArc(GX, GY, GAUGE_R, 5.6, P, 40),
  knurl: knurl(GX, GY, GAUGE_R, 5.6, GAUGE_TOP, 72, P, 0.2),
  bezel: ring(GX, GY, GAUGE_R - 1.6, GAUGE_TOP, P, 64),
  face: ring(GX, GY, GAUGE_R - 2.6, GAUGE_TOP, P, 64),
  minor: DIAL.minor,
  major: DIAL.major,
  limit: lineOnTop(dialPoint(1, GAUGE_R - 2.6), dialPoint(1, GAUGE_R - 6), GAUGE_TOP, P),
  label: topMatrix([GX - 3.4, GY + 5.2], GAUGE_TOP, P),
  crown: slab({ x: GX + GAUGE_R - 0.4, y: GY - 1.4, w: 3, d: 2.8, r: 0.6 }, 4, 2.6, 2),
  crownCap: cyl(GX + GAUGE_R + 3.6, GY, 1.8, 3.4, 3.8, 18),
  hub: ring(GX, GY, 1.2, GAUGE_TOP + 0.2, P, 16),
  hubDot: [at([GX, GY, GAUGE_TOP + 0.2])],
}

export function needleAt(tokens: number) {
  const share = Math.min(1, Math.max(0, tokens / LIMITS.tokens))
  return lineOnTop(dialPoint(share, -2.2), dialPoint(share, GAUGE_R - 3.4), GAUGE_TOP + 0.2, P)
}

// Tickets: built inside the printer at HOME_X, moved along the rail by the live loop.
const PAPER: Plan = { x: HOME_X - PAPER_W / 2, y: PAPER_Y, w: PAPER_W, d: PAPER_D, r: 0 }
const PAPER_FACE = PAPER_Y + PAPER_D
// A steel ball in a cage on the rider's face pinches the ticket, as on a kitchen ticket rail.
const BALL_R = 2.6
const CAGE_FRONT = PAPER_FACE + BALL_R * 2 + 0.2
const CAGE_BACK = PAPER_FACE + 0.2
const BALL: Point3 = [HOME_X, PAPER_FACE + BALL_R, PAPER_TOP - 1.8]
const LEDGE_Z = BALL[2] - BALL_R
const ROOF_Z = BALL[2] + BALL_R
const cheek = (side: -1 | 1): Plan => ({ x: HOME_X + side * (BALL_R + 0.2) - (side < 0 ? 0.7 : 0), y: CAGE_BACK, w: 0.7, d: CAGE_FRONT - CAGE_BACK, r: 0 })
const left = HOME_X - PAPER_W / 2 + 2
const ballCentre = at(BALL)
export const BALL_ART = { cx: ballCentre[0], cy: ballCentre[1], r: BALL_R * P.scale * Math.sqrt(1.6) }
export const CAGE_FRONT_Y = CAGE_FRONT
export const CAGE_ROOF = ROOF_Z + 0.8
export const TICKET_ART = TICKETS.map((ticket) => ({
  id: ticket.id,
  round: ticket.round,
  verdict: ticket.verdict,
  rider: slab(RIDER, RIDER_Z, RIDER_H, 3, 0.4),
  riderSeam: sideSeam(RIDER, RIDER_Z + 2.4, P, 3),
  riderScrews: [at([HOME_X - 3, 0, RIDER_Z + RIDER_H]), at([HOME_X + 3, 0, RIDER_Z + RIDER_H])],
  paper: slab(PAPER, PAPER_Z, PAPER_TOP - PAPER_Z, 1),
  // The number sits under the ball cage, which hangs in front of the paper and hides the top-left corner.
  number: sideMatrix([left, PAPER_FACE], 61.6, P, 'left'),
  rules: join([0, 1].map((row) => onFaceY(PAPER_FACE, [[left, 58.4 - row * 2.8], [left + [15, 11][row]!, 58.4 - row * 2.8]], false))),
  holes: [0, 1].map((index) => onFaceY(PAPER_FACE, roundRect(HOME_X + 4.6 + index * 3.2, HOME_X + 6.4 + index * 3.2, PAPER_TOP - 4.2, PAPER_TOP - 2.4, 0.9))),
  stamp: onFaceY(PAPER_FACE, roundRect(HOME_X - 1, HOME_X + 10, PAPER_Z + 1.4, PAPER_Z + 6.8, 0.9)),
  // A tick for accepted, a hook back for rework: the stamp's two faces.
  mark:
    ticket.verdict === 'accept'
      ? onFaceY(PAPER_FACE, [[HOME_X + 2, PAPER_Z + 4.2], [HOME_X + 4, PAPER_Z + 2.6], [HOME_X + 7.6, PAPER_Z + 5.8]], false)
      : onFaceY(PAPER_FACE, [[HOME_X + 7.6, PAPER_Z + 2.8], [HOME_X + 2.4, PAPER_Z + 2.8], [HOME_X + 2.4, PAPER_Z + 5.4], [HOME_X + 4.4, PAPER_Z + 5.4]], false),
  cheekBack: slab(cheek(-1), LEDGE_Z - 0.8, ROOF_Z - LEDGE_Z + 0.8, 1),
  ledge: slab({ x: HOME_X - BALL_R - 0.2, y: CAGE_BACK, w: (BALL_R + 0.2) * 2, d: CAGE_FRONT - CAGE_BACK, r: 0 }, LEDGE_Z - 0.8, 0.8, 1),
  roof: slab({ x: HOME_X - BALL_R - 0.9, y: RIDER.y + RIDER.d, w: (BALL_R + 0.9) * 2, d: CAGE_FRONT - RIDER.y - RIDER.d, r: 0.4 }, ROOF_Z, 0.8, 2, 0.2),
  roofScrews: [at([HOME_X - BALL_R - 0.2, CAGE_FRONT - 1, ROOF_Z + 0.8]), at([HOME_X + BALL_R + 0.2, CAGE_FRONT - 1, ROOF_Z + 0.8])],
  cheekFront: slab(cheek(1), LEDGE_Z - 0.8, ROOF_Z - LEDGE_Z + 0.8, 1),
}))

/** Screen region in front of the printer's mouth: the part of a ticket that has left the printer. */
export const MOUTH_CLIP = (() => {
  const low = at([FACE_X, CAGE_FRONT, -60])
  const jaw = at([FACE_X, CAGE_FRONT, CAGE_ROOF])
  const face = at([FACE_X, RIDER.y + RIDER.d, CAGE_ROOF])
  const crown = at([FACE_X, RIDER.y + RIDER.d, RIDER_Z + RIDER_H])
  const back = at([FACE_X, RIDER.y, RIDER_Z + RIDER_H])
  const high = at([FACE_X, RIDER.y, 400])
  const far = WIDTH * 2
  const points: Flat[] = [low, jaw, face, crown, back, high, [far, high[1]], [far, HEIGHT * 2], [low[0], HEIGHT * 2]]
  return pathOf(points, true)
})()

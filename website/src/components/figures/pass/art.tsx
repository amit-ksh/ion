import { Dots, FaceText, Line, Solid } from '@/components/iso/draw'
import { BALL_ART, COUNTER_ART, DUCT_ART, FRAME_ART, GAUGE_ART, HOPPER_ART, PRINTER_ART, RAIL_ART, STAMP_ART, STATION_ART, TICKET_ART, TRAY_ART } from './geometry'

export const COUNTER = (
  <g key="counter">
    <path className="iso-halo" d={COUNTER_ART.halo} />
    {COUNTER_ART.feet.map((parts, index) => (
      <g key={index}>
        <Solid paths={parts[0]!} tone="lo" />
        <Solid paths={parts[1]!} tone="mid" />
      </g>
    ))}
    <Solid paths={COUNTER_ART.slab} tone="mid" inner={<Line d={COUNTER_ART.band} tone="faint" />} />
    <Line d={COUNTER_ART.groove} tone="lo" />
    <Line d={COUNTER_ART.screwRings} tone="lo" />
    <Dots points={COUNTER_ART.screws} size={0.5} tone="mid" />
    <Solid paths={COUNTER_ART.label} tone="lo" crease="none" />
    <Line d={COUNTER_ART.labelLine} tone="faint" />
    <Line d={COUNTER_ART.labelRules} tone="lo" free />
    <Dots points={COUNTER_ART.labelScrews} size={0.45} tone="mid" />
  </g>
)

export const PRINTER = (
  <g key="printer">
    <Solid paths={PRINTER_ART.body} tone="mid" inner={<Line d={PRINTER_ART.seams} tone="faint" />} />
    <path className="iso-shade pass-void" d={PRINTER_ART.mouth} />
    <Line d={PRINTER_ART.mouthLip} tone="faint" />
    <Line d={PRINTER_ART.mouth} tone="lo" />
    <Line d={PRINTER_ART.vents} tone="lo" />
    <Solid paths={PRINTER_ART.plate} tone="lo" crease="none" />
    <Line d={PRINTER_ART.plateRules} tone="lo" free />
    <Dots points={PRINTER_ART.plateScrews} size={0.45} tone="mid" />
    <Solid paths={PRINTER_ART.cap} tone="mid" />
    <Line d={PRINTER_ART.lid} tone="faint" />
    <Dots points={PRINTER_ART.hinges} size={0.5} tone="mid" />
    <Solid paths={PRINTER_ART.button} tone="mid" />
    <Line d={PRINTER_ART.buttonRing} tone="lo" />
    <Solid paths={PRINTER_ART.lampBase} tone="lo" />
    <g className="pass-lamp" data-lamp="printer">
      <Solid paths={PRINTER_ART.lamp} tone="mid" />
    </g>
  </g>
)

export const DUCT = (
  <g key="duct">
    <Solid paths={DUCT_ART.body} tone="mid" />
    <Line d={DUCT_ART.bands} tone="faint" />
    <Solid paths={DUCT_ART.hatch} tone="lo" crease="none" />
    <Dots points={DUCT_ART.hatchScrews} size={0.45} tone="mid" />
  </g>
)

const boxBack = (art: typeof HOPPER_ART, key: string) => (
  <g key={key}>
    <Solid paths={art.floor} tone="lo" crease="none" />
    <Solid paths={art.back} tone="mid" />
    <Solid paths={art.left} tone="mid" />
  </g>
)
const boxFront = (art: typeof HOPPER_ART, key: string) => (
  <g key={key}>
    <Solid paths={art.right} tone="mid" />
    <Solid paths={art.front} tone="mid" />
    <Dots points={art.rivets} size={0.45} tone="mid" />
  </g>
)
export const HOPPER_BACK = boxBack(HOPPER_ART, 'hopper-back')
export const HOPPER_FRONT = boxFront(HOPPER_ART, 'hopper-front')
export const TRAY_BACK = boxBack(TRAY_ART, 'tray-back')
export const TRAY_FRONT = boxFront(TRAY_ART, 'tray-front')

export const FRAME = (
  <g key="frame">
    {FRAME_ART.feet.map((paths, index) => (
      <Solid key={`foot${index}`} paths={paths} tone="lo" />
    ))}
    <Dots points={FRAME_ART.footScrews} size={0.45} tone="mid" />
    {FRAME_ART.posts.map((paths, index) => (
      <Solid key={`post${index}`} paths={paths} tone="mid" />
    ))}
    {FRAME_ART.brackets.map((paths, index) => (
      <Solid key={`bracket${index}`} paths={paths} tone="mid" />
    ))}
    <Solid paths={FRAME_ART.anvil} tone="mid" />
    <Line d={FRAME_ART.anvilInset} tone="faint" />
    <Dots points={FRAME_ART.anvilScrews} size={0.5} tone="mid" />
  </g>
)

export const RAIL = (
  <g key="rail">
    <Solid paths={RAIL_ART.postFoot} tone="lo" />
    <Dots points={RAIL_ART.postScrews} size={0.45} tone="mid" />
    <Solid paths={RAIL_ART.post} tone="mid" inner={<Line d={RAIL_ART.postSeam} tone="faint" />} />
    <Solid paths={RAIL_ART.collar} tone="mid" />
    <Solid paths={RAIL_ART.rail} tone="mid" />
    <Line d={RAIL_ART.ruleMinor} tone="lo" />
    <Line d={RAIL_ART.ruleMajor} tone="mid" />
    <Line d={RAIL_ART.gates} tone="faint" />
    <Dots points={RAIL_ART.gatePins} size={0.5} tone="mid" />
    <Solid paths={RAIL_ART.stop} tone="mid" />
    <Dots points={[RAIL_ART.stopScrew]} size={0.45} tone="mid" />
  </g>
)

export const TICKETS = TICKET_ART.map((art) => (
  <g key={art.id}>
    <Solid paths={art.rider} tone="mid" inner={<Line d={art.riderSeam} tone="faint" />} />
    <Dots points={art.riderScrews} size={0.45} tone="mid" />
    <g className="pass-paper">
      <Solid paths={art.paper} tone="mid" crease="none" />
    </g>
    <FaceText transform={art.number} size={3.2} tone="hi">
      {`#${art.id}`}
    </FaceText>
    <Line d={art.rules} tone="lo" free />
    {art.holes.map((d, index) => (
      <path key={index} className={index < art.round ? 'iso-line pass-punched' : 'iso-line'} data-tone="lo" d={d} />
    ))}
    <g className="pass-ink" data-verdict={art.verdict} opacity={0}>
      <Line d={art.stamp} tone="hi" />
      <Line d={art.mark} tone="hi" free />
    </g>
    <Solid paths={art.cheekBack} tone="mid" crease="none" />
    <Solid paths={art.ledge} tone="mid" crease="none" />
    <circle className="pass-ball" cx={BALL_ART.cx} cy={BALL_ART.cy} r={BALL_ART.r} />
    <Solid paths={art.roof} tone="mid" />
    <Dots points={art.roofScrews} size={0.4} tone="mid" />
    <Solid paths={art.cheekFront} tone="mid" />
  </g>
))

export const STATIONS = (
  <g key="stations">
    {STATION_ART.map((art, index) => (
      <g key={index}>
        <Solid paths={art.body} tone="mid" inner={<Line d={art.seam} tone="faint" />} />
        <Solid paths={art.top} tone="mid" />
        <Line d={art.slot} tone="faint" />
        <Solid paths={art.knob} tone="mid" inner={<Line d={art.knurl} tone="lo" />} />
        <Line d={art.pointer} tone="hi" free />
        <Solid paths={art.lampBase} tone="lo" />
        <g className="pass-lamp" data-lamp={index}>
          <path className="pass-glow" d={art.lamp.fill} />
          <Solid paths={art.lamp} tone="mid" />
        </g>
        <Solid paths={art.plate} tone="lo" crease="none" />
        <Line d={art.plateRules} tone="lo" free />
        <Dots points={art.plateScrews} size={0.4} tone="mid" />
      </g>
    ))}
  </g>
)

export const PLUNGER = (
  <g key="plunger">
    <Solid paths={STAMP_ART.rod} tone="mid" crease="none" />
    <Solid paths={STAMP_ART.pad} tone="mid" />
  </g>
)

export const STAMP = (
  <g key="stamp">
    <Solid paths={STAMP_ART.housing} tone="mid" inner={<Line d={STAMP_ART.housingSeam} tone="faint" />} />
    <Solid paths={STAMP_ART.foot} tone="lo" />
    <Dots points={STAMP_ART.footScrews} size={0.45} tone="mid" />
    <Solid paths={STAMP_ART.post} tone="mid" crease="none" inner={<Line d={STAMP_ART.postSeam} tone="faint" />} />
    <Solid paths={STAMP_ART.collar} tone="mid" />
    <Solid paths={STAMP_ART.arm} tone="mid" />
    <Dots points={[STAMP_ART.armScrew]} size={0.5} tone="mid" />
  </g>
)

export const GAUGE = (
  <g key="gauge">
    <Solid paths={GAUGE_ART.flange} tone="lo" />
    <Dots points={GAUGE_ART.flangeScrews} size={0.45} tone="mid" />
    <Solid paths={GAUGE_ART.puck} tone="mid" inner={<Line d={GAUGE_ART.band + GAUGE_ART.knurl} tone="faint" />} />
    <Line d={GAUGE_ART.bezel} tone="lo" />
    <Line d={GAUGE_ART.face} tone="faint" />
    <Line d={GAUGE_ART.minor} tone="lo" />
    <Line d={GAUGE_ART.major} tone="mid" />
    <Line d={GAUGE_ART.limit} tone="hi" />
    <FaceText transform={GAUGE_ART.label} size={2.1} tone="lo">
      TOKENS
    </FaceText>
    <Solid paths={GAUGE_ART.crown} tone="mid" />
    <Solid paths={GAUGE_ART.crownCap} tone="mid" />
  </g>
)

export const HUB = (
  <g key="hub">
    <Line d={GAUGE_ART.hub} tone="hi" />
    <Dots points={GAUGE_ART.hubDot} size={0.7} tone="hi" />
  </g>
)

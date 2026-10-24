import { Plate } from '@/components/iso/draw'
import { PassLive } from './live'
import { LIMITS, RUN_TOTAL, STILL, readoutAt } from './model'

const fmt = (value: number) => value.toLocaleString('en-US')

export function PassFigure() {
  return (
    <div className="iso pass" data-theme="light">
      <Plate
        fig="Fig 1"
        title="The pass"
        hint={
          <>
            <span className="pass-hint-fine">Hover to scrub · ← → to step</span>
            <span className="pass-hint-touch">Tap to step</span>
          </>
        }
        readout={readoutAt(STILL)}
        keys={[
          { mark: 'lit', label: 'Result under review at the pass' },
          { mark: 'raised', label: 'Agent station, lamp lit while it runs' },
          { mark: 'edge', label: 'Needle: tokens spent against the run budget' },
        ]}
        caption={`One team run, drawn on Ion's real default limits: at most ${LIMITS.parallel} assignments run at once, every child result is reviewed by its parent, a rejected result is superseded by a linked replacement for the same agent (at most ${LIMITS.reworkRounds} rework rounds), and the run is capped at ${fmt(LIMITS.tokens)} tokens. Ticket numbers, station pairings and the ${fmt(Math.round(RUN_TOTAL / 1000) * 1000)}-token spend are an illustration.`}
      >
        <PassLive />
      </Plate>
    </div>
  )
}

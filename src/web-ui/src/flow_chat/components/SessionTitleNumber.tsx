import './SessionTitleNumber.scss';

/** Auxiliary identity stays outside the title's overflow and edit slots. */
export function SessionTitleNumber({ number }: { number?: string }) {
  return number ? (
    <span
      className="openbitfun-session-title-number"
      data-openbitfun-component="session-title-number"
      data-openbitfun-part="root"
    >{number}</span>
  ) : null;
}

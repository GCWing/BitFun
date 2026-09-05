import { useId } from 'react';
import artwork from '../../../../../../assets/brand/source/release-letter-mascot.svg?no-inline';

export default function ReleaseLetterMascot() {
  const shadowId = useId();
  return (
    <svg className="release-letter__mascot" viewBox="-7 -4 120 146" aria-hidden="true" focusable="false">
      <defs>
        <filter id={shadowId} x="-70%" y="-180%" width="240%" height="460%">
          <feGaussianBlur data-mascot="blur" stdDeviation="3.33" />
        </filter>
      </defs>
      <ellipse data-mascot="shadow" cx="53" cy="134.8" rx="36.04" ry="3" fill="var(--openbitfun-color-content-on-light)" opacity=".2" filter={`url(#${shadowId})`} />
      <g data-mascot="lift">
        <g data-mascot="body">
          <g data-mascot="deform">
            <use href={`${artwork}#body`} />
            <g data-mascot="eyeOne"><g className="release-letter__mascot-eye"><use href={`${artwork}#eyeOne`} /></g></g>
            <g data-mascot="eyeTwo"><g className="release-letter__mascot-eye"><use href={`${artwork}#eyeTwo`} /></g></g>
          </g>
          <g data-mascot="head" transform="translate(.035 0)"><use href={`${artwork}#head`} /></g>
        </g>
      </g>
    </svg>
  );
}

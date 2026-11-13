import type { ReactNode } from "react";

/** The mascot's outline, traced from the artwork (potrace units: 0.1 px, y up, in a 2000 px frame). */
const BODY = "M7934 14590 c-271 -40 -524 -239 -714 -560 -160 -270 -501 -1042 -732 -1655 -148 -392 -179 -405 -567 -226 -532 245 -1621 664 -1650 635 -11 -11 -13 -5298 -2 -5654 24 -746 102 -1121 321 -1537 288 -548 797 -977 1385 -1166 314 -101 594 -140 1135 -158 366 -12 5804 -9 5816 4 8 8 8 10 -68 217 -160 440 -302 790 -468 1155 -215 471 -210 485 241 649 547 198 1455 609 1694 767 142 94 300 251 367 365 138 235 153 493 43 730 -103 224 -308 412 -623 573 -385 195 -1025 473 -1498 649 -428 160 -434 177 -242 602 288 635 595 1447 660 1745 32 147 32 404 -1 525 -112 415 -466 638 -937 591 -156 -15 -292 -50 -539 -136 -530 -186 -801 -295 -1495 -602 -103 -46 -138 -57 -196 -61 -123 -8 -150 30 -294 408 -202 535 -528 1273 -685 1550 -241 426 -590 643 -951 590z m2269 -4221 c106 -18 313 -98 331 -128 4 -5 -17 -17 -46 -25 -218 -66 -352 -328 -278 -545 107 -319 506 -406 733 -161 l56 60 7 -36 c13 -73 -3 -324 -26 -404 -102 -346 -361 -602 -705 -697 -130 -36 -353 -40 -475 -9 -367 92 -625 340 -738 706 -37 121 -39 380 -4 510 136 506 627 819 1145 729z m-2269 -553 c80 -24 218 -85 239 -106 5 -4 -8 -13 -30 -20 -105 -34 -188 -96 -243 -182 -128 -197 -73 -467 121 -597 86 -58 150 -74 269 -69 123 5 199 39 289 129 l60 61 7 -37 c15 -81 -2 -315 -30 -408 -192 -642 -935 -919 -1494 -557 -504 326 -604 1025 -212 1479 141 163 331 277 538 322 133 29 364 22 486 -15z";
const FRAME = "translate(0 2000) scale(0.1 -0.1)";

/** The mascot on its light tile: the app icon, small. */
export function MascotMark({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="426 426 1148 1148" aria-hidden="true" data-mascot-mark>
      <defs>
        <clipPath id="mascot-tile">
          <rect x="426" y="426" width="1148" height="1148" rx="256" />
        </clipPath>
      </defs>
      <rect x="426" y="426" width="1148" height="1148" rx="256" fill="#EDEDED" />
      <g clipPath="url(#mascot-tile)" fill="#0B0B0C">
        <rect x="380" y="760" width="60" height="860" />
        <rect x="380" y="1560" width="890" height="80" />
        <g transform={FRAME}>
          <path d={BODY} />
        </g>
      </g>
    </svg>
  );
}

/**
 * The mascot peeking in from a corner: a dim silhouette with light eyes, so it sits in the dark
 * interface without shouting. The eye whites are drawn under the body, which has holes for them.
 */
export function Mascot({ size = 96, className = "" }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size * (1038 / 1058)} viewBox="426 536 1058 1038" className={className} aria-hidden="true" data-mascot>
      <circle cx="761" cy="1124" r="112" fill="var(--color-text)" />
      <circle cx="1005" cy="1057" r="106" fill="var(--color-text)" />
      <g transform={FRAME} fill="var(--color-line-strong)">
        <path d={BODY} />
      </g>
    </svg>
  );
}

/** An empty state: the mascot in the bottom left corner, looking up at the line beside it. */
export function Empty({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="relative flex min-h-[112px] items-start overflow-hidden pb-4 pl-[104px] pr-4 pt-5">
      <Mascot size={88} className="absolute bottom-0 left-0" />
      <div className="flex min-w-0 flex-col items-start gap-2 text-[12px] text-text-3">
        <p>{children}</p>
        {action}
      </div>
    </div>
  );
}

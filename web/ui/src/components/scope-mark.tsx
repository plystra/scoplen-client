// SPDX-License-Identifier: Apache-2.0

/**
 * The mark's two corners drawn around a selected item: the top-left and
 * bottom-right corners of its box, never a filled bar or a ring. Place it
 * inside a relatively positioned element; it is decorative, so the element
 * itself must convey selection to assistive technology (`aria-selected`,
 * `aria-current`).
 */
export function ScopeMark({ inset = 0 }: { inset?: number }) {
  const corner = "absolute size-2.5 border-primary";
  return (
    <span aria-hidden="true" className="pointer-events-none absolute" style={{ inset }}>
      <span className={`${corner} top-0 left-0 rounded-tl-[3px] border-t-2 border-l-2`} />
      <span className={`${corner} right-0 bottom-0 rounded-br-[3px] border-r-2 border-b-2`} />
    </span>
  );
}

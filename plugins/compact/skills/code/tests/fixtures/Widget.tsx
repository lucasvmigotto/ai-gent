import React from "react";

type Props = { title: string; items: string[]; onPick?: (s: string) => void };

export function Widget({ title, items, onPick }: Props) {
  const count = items.length;
  return (
    <section className="widget">
      <h2>
        {title} ({count})
      </h2>
      <p>
        Some text that
        spans lines
      </p>
      <ul>
        {items.map((it) => (
          <li key={it} onClick={() => onPick?.(it)}>
            {it}
          </li>
        ))}
      </ul>
      {count > 3 ? <b>many</b> : <i>few</i>}
    </section>
  );
}

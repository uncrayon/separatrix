import { Tex } from "./Tex";

/**
 * A sentence with its maths between `$…$` typeset, the rest as text:
 * "The cobweb view needs a one-dimensional map $x_{n+1} = f(x_n)$; …".
 */
export function MathProse({ text }: { text: string }) {
  return (
    <>
      {text.split("$").map((part, i) =>
        i % 2 === 1 ? (
          <span className="math" key={i}>
            <Tex latex={part} />
          </span>
        ) : (
          part
        ),
      )}
    </>
  );
}

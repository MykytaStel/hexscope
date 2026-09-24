// "How DEFLATE works, visually": the player, in text mode, on a short demo
// text, with buttons in the story that move it to the step being described.
import "./style.css";
import "./story.css";
import { StepKind, STRIDE } from "./deflate";
import { FileModel } from "./model";
import { Player } from "./player";
import { call, playerSource } from "./rpc";

const host = document.getElementById("story-player")!;
const player = new Player(host, { onHead: () => {}, onClose: () => {} }, true);

window.addEventListener("keydown", (e) => {
  if (player.handleKey(e)) e.preventDefault();
});

const fmt = (n: number) => n.toLocaleString();

/**
 * Places the copy diagram's boxes and arc on the characters themselves, so
 * they line up whatever monospace font the reader has.
 */
function drawCopyDiagram(): void {
  const svg = document.getElementById("copy-diagram");
  const text = svg?.querySelector<SVGTextElement>(".d-text");
  if (!svg || !text) return;
  const box = (from: number, len: number) => {
    const x = text.getStartPositionOfChar(from).x;
    return { x: x - 3, w: text.getSubStringLength(from, len) + 6 };
  };
  // "byte" at 18, and the copy of it at 28: ten bytes back, four long.
  const src = box(18, 4);
  const dst = box(28, 4);
  const place = (sel: string, b: { x: number; w: number }) => {
    const r = svg.querySelector<SVGRectElement>(sel)!;
    r.setAttribute("x", String(b.x));
    r.setAttribute("y", "64");
    r.setAttribute("width", String(b.w));
    r.setAttribute("height", "28");
  };
  place(".d-src", src);
  place(".d-dst", dst);
  const a = src.x + src.w / 2;
  const b = dst.x + dst.w / 2;
  svg.querySelector(".d-arc")!.setAttribute("d", `M${a} 60 Q ${(a + b) / 2} 10 ${b} 60`);
  svg.querySelector(".d-arrow")!.setAttribute("d", `M${b} 61 l-6 -9 h12 z`);
  svg.querySelector(".d-label")!.setAttribute("x", String((a + b) / 2));
}
drawCopyDiagram();
document.fonts?.ready.then(drawCopyDiagram);

async function start(): Promise<void> {
  const bytes = new Uint8Array(await (await fetch("samples/deflate-demo.zip")).arrayBuffer());
  const parsed = await call({ type: "parse", file: new File([bytes], "deflate-demo.zip") });
  if (parsed.type !== "parsed") throw new Error("could not read the demo");
  const model = new FileModel(parsed.result, bytes, "rain.txt");
  const stream = await call({ type: "selectEntry", index: 0 });
  if (stream.type !== "stream" || !stream.playable) throw new Error("could not play the demo");
  model.setStream(stream.segments, stream.trace, stream.idatBytes);
  host.querySelector(".story-loading")?.remove();
  await player.open(model, playerSource);

  // Steps worth showing, found once.
  const [total, literals, matches, output] = stream.trace ?? [0, 0, 0, 0];
  const steps = await playerSource.steps(0, total);
  let copy = -1;
  let codes = -1;
  for (let i = 0; i * STRIDE < steps.length; i++) {
    const kind = steps[i * STRIDE];
    const length = steps[i * STRIDE + 2];
    if (kind === StepKind.Match && length >= 8 && copy < 0) copy = i;
    if (kind === StepKind.Literal && codes < 0 && i > 3) codes = i;
  }
  const targets: Record<string, number> = { copy, codes, header: 0, end: total - 1 };

  const compressed = stream.idatBytes;
  document.getElementById("stat-size")!.textContent = `${fmt(output)} bytes of text, stored in ${fmt(compressed)}`;
  document.getElementById("stat-summary")!.textContent =
    `Put together, these two ideas are all there is. The ${fmt(output)} bytes of text in the player take ` +
    `${fmt(compressed)}: ${fmt(literals)} characters written out, and ${fmt(matches)} copies from earlier ` +
    `standing in for the other ${fmt(output - literals)} bytes.`;

  for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-jump]")) {
    const target = targets[btn.dataset.jump!] ?? -1;
    btn.disabled = target < 0;
    btn.addEventListener("click", () => {
      player.jump(target);
    });
  }
  player.jump(0);
}

start().catch((err: unknown) => {
  host.textContent = `The demo could not load: ${err instanceof Error ? err.message : String(err)}`;
});

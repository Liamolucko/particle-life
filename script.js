// @ts-check

import init, { Settings, Universe } from "./pkg/particle_life_web.js";

init().then(async () => {
  const canvas = document.getElementById("canvas");
  if (!(canvas instanceof HTMLCanvasElement)) {
    throw new Error("Expected `canvas` to be a canvas");
  }
  const ctx = canvas.getContext("2d", { alpha: false });

  canvas.width = canvas.offsetWidth;
  canvas.height = canvas.offsetHeight
  const universe = Universe.new(canvas.width, canvas.height);

  universe.wrap = true;

  await universe.seed(9, 400, Settings.balanced());

  window.addEventListener("keydown", ev => {
    switch (ev.key) {
      case "w":
        universe.wrap = !universe.wrap;
        break;
    }
  })

  function draw() {
    ctx.fillStyle = "black";
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    for (let opacity = 0.1; opacity <= 1; opacity += 0.1) {
      universe.step();
      universe.draw(ctx, opacity);
    }

    requestAnimationFrame(draw);
  }

  requestAnimationFrame(draw);
});

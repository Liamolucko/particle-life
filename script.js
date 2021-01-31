// @ts-check

import init, { Settings, Universe } from "./pkg/particle_life_web.js";

init().then(() => {
  const canvas = document.getElementById("canvas");
  if (!(canvas instanceof HTMLCanvasElement)) {
    throw new Error("Expected `canvas` to be a canvas");
  }
  const ctx = canvas.getContext("2d", { alpha: false });

  const universe = Universe.new(1600, 900);

  universe.wrap = true;

  universe.seed(9, 400, Settings.balanced());

  function draw() {
    for (let i = 0; i < 10; i++) universe.step();

    universe.draw(ctx);

    requestAnimationFrame(draw);
  }

  requestAnimationFrame(draw);
});

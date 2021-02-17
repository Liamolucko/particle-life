importScripts("./pkg/particle_life.js");

wasm_bindgen("./pkg/particle_life_bg.wasm").then(() => {
  wasm_bindgen.run_worker();
});

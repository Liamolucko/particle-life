if ("SharedArrayBuffer" in globalThis) {
  importScripts("./sab/particle_life.js");

  self.onmessage = (event) => {
    let initialised = wasm_bindgen(...event.data);

    self.onmessage = async (event) => {
      await initialised;
      wasm_bindgen.run_worker_sab(...event.data);
    };
  };
} else {
  importScripts("./no-sab/particle_life.js");

  wasm_bindgen("./no-sab/particle_life_bg.wasm").then(() => {
    wasm_bindgen.run_worker();
  });
}

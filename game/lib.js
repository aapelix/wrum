mergeInto(LibraryManager.library, {
  init_ws: function () {
    ws_on_message = Module.cwrap("ws_on_message", "void", ["number", "number"]);

    ws_on_open = Module.cwrap("ws_on_open", "void", []);
    ws_on_close = Module.cwrap("ws_on_close", "void", []);

    const ws = new WebSocket("ws://localhost:3000");

    ws.onopen = () => {
      console.debug("ws connected");
      ws_on_open();
    };

    ws.onerror = (e) => console.error("ws error: ", e);
    ws.onclose = (e) => {
      console.debug("ws closed");
      ws_on_close();
    };
    ws.onmessage = (event) => {
      const text = event.data;
      console.debug("ws message: ", text);
      const encoder = new TextEncoder();
      const bytes = encoder.encode(text);
      const ptr = Module._malloc(bytes.length);
      Module.HEAPU8.set(bytes, ptr);
      ws_on_message(ptr, bytes.length);
      Module._free(ptr);
    };

    Module.ws = ws;
  },
  send_ws: function (ptr, len) {
    if (!Module.ws || Module.ws.readyState !== WebSocket.OPEN) {
      console.error("ws not open");
      return;
    }

    const data = new Uint8Array(Module.HEAPU8.buffer, ptr, len);
    Module.ws.send(data);
  },
});

mergeInto(LibraryManager.library, {
  ws_open: function (urlPtr) {
    var url = UTF8ToString(urlPtr);
    if (url.startsWith("/")) {
      url =
        (location.protocol === "https:" ? "wss://" : "ws://") +
        location.host +
        url;
    }
    if (Module.__ws && Module.__ws.s) {
      try {
        Module.__ws.s.close();
      } catch (e) {}
    }
    var w = { q: [], state: 0, s: null };
    Module.__ws = w;
    var s = new WebSocket(url);
    s.binaryType = "arraybuffer";
    s.onopen = function () {
      w.state = 1;
    };
    s.onmessage = function (e) {
      w.q.push(new Uint8Array(e.data));
    };
    s.onclose = s.onerror = function () {
      w.state = 2;
    };
    w.s = s;
  },
  ws_state: function () {
    return Module.__ws ? Module.__ws.state : 2;
  },
  ws_send: function (ptr, len) {
    var w = Module.__ws;
    if (!w || w.state !== 1) return -1;
    w.s.send(HEAPU8.slice(ptr, ptr + len));
    return 0;
  },
  ws_poll: function (ptr, cap) {
    var w = Module.__ws;
    if (!w || w.q.length === 0) return 0;
    var m = w.q.shift();
    if (m.length > cap) return -1;
    HEAPU8.set(m, ptr);
    return m.length;
  },
  ws_close: function () {
    var w = Module.__ws;
    if (w && w.s) {
      try {
        w.s.close();
      } catch (e) {}
    }
  },
});

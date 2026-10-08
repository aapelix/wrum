mergeInto(LibraryManager.library, {
  $TN: { next: 1, socks: {} },

  tn_ws_open__deps: ["$TN", "$UTF8ToString"],
  tn_ws_open: function (urlPtr) {
    var s = { ws: null, inbox: [], state: 0 };
    var id = TN.next++;
    TN.socks[id] = s;
    try {
      s.ws = new WebSocket(UTF8ToString(urlPtr));
    } catch (e) {
      s.state = 2;
      return id;
    }
    s.ws.binaryType = "arraybuffer";
    s.ws.onopen = function () {
      s.state = 1;
    };
    s.ws.onmessage = function (e) {
      if (e.data instanceof ArrayBuffer) s.inbox.push(new Uint8Array(e.data));
    };
    s.ws.onclose = function () {
      s.state = 2;
    };
    s.ws.onerror = function () {
      s.state = 2;
    };
    return id;
  },

  tn_ws_state__deps: ["$TN"],
  tn_ws_state: function (id) {
    var s = TN.socks[id];
    return s ? s.state : 2;
  },

  tn_ws_send__deps: ["$TN"],
  tn_ws_send: function (id, ptr, len) {
    var s = TN.socks[id];
    if (!s || s.state !== 1) return -1;
    s.ws.send(HEAPU8.slice(ptr, ptr + len));
    return 0;
  },

  tn_ws_recv__deps: ["$TN"],
  tn_ws_recv: function (id, ptr, cap) {
    var s = TN.socks[id];
    if (!s || s.inbox.length === 0) return -1;
    var m = s.inbox.shift();
    if (m.length > cap) return -2;
    HEAPU8.set(m, ptr);
    return m.length;
  },

  tn_ws_close__deps: ["$TN"],
  tn_ws_close: function (id) {
    var s = TN.socks[id];
    if (s) {
      try {
        if (s.ws) s.ws.close();
      } catch (e) {}
      delete TN.socks[id];
    }
  },
});

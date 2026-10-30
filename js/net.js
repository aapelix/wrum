mergeInto(LibraryManager.library, {
  net_open: function (wsPtr, wtPtr, hashPtr) {
    var wsUrl = UTF8ToString(wsPtr),
      wtArg = UTF8ToString(wtPtr),
      hash = UTF8ToString(hashPtr);
    if (wsUrl.startsWith("/"))
      wsUrl =
        (location.protocol === "https:" ? "wss://" : "ws://") +
        location.host +
        wsUrl;
    if (Module.__net) {
      try {
        Module.__net.close();
      } catch (e) {}
    }
    var n = {
      q: [],
      state: 0,
      kind: 0,
      send: function () {
        return -1;
      },
      close: function () {},
    };
    Module.__net = n;

    function useWs() {
      if (Module.__net !== n) return;
      var s = new WebSocket(wsUrl);
      s.binaryType = "arraybuffer";
      s.onopen = function () {
        n.kind = 1;
        n.state = 1;
      };
      s.onmessage = function (e) {
        n.q.push(new Uint8Array(e.data));
      };
      s.onclose = s.onerror = function () {
        n.state = 2;
      };
      n.send = function (ch, b) {
        if (s.readyState !== 1) return -1;
        s.send(b);
        return 0;
      };
      n.close = function () {
        try {
          s.close();
        } catch (e) {}
      };
    }

    async function readFramed(stream) {
      var r = stream.getReader(),
        buf = new Uint8Array(0);
      for (;;) {
        var x = await r.read();
        if (x.done) return;
        var m = new Uint8Array(buf.length + x.value.length);
        m.set(buf);
        m.set(x.value, buf.length);
        buf = m;
        while (buf.length >= 4) {
          var len = new DataView(buf.buffer, buf.byteOffset).getUint32(0, true);
          if (buf.length < 4 + len) break;
          n.q.push(buf.slice(4, 4 + len));
          buf = buf.subarray(4 + len);
        }
      }
    }

    async function useWt() {
      var t = null;
      try {
        var opts = {};
        if (hash) {
          var v = new Uint8Array(hash.length / 2);
          for (var i = 0; i < v.length; i++)
            v[i] = parseInt(hash.substr(i * 2, 2), 16);
          opts.serverCertificateHashes = [{ algorithm: "sha-256", value: v }];
        }
        t = new WebTransport(
          "https://" + location.hostname + wtArg + "/",
          opts,
        );
        await Promise.race([
          t.ready,
          new Promise(function (_, rej) {
            setTimeout(function () {
              rej(new Error("wt timeout"));
            }, 3000);
          }),
        ]);
        var dgw = t.datagrams.writable.getWriter();
        var uw = (await t.createUnidirectionalStream()).getWriter();
        n.send = function (ch, b) {
          if (ch === 1 && b.length <= t.datagrams.maxDatagramSize) {
            dgw.write(b).catch(function () {});
            return 0;
          }
          var f = new Uint8Array(4 + b.length);
          new DataView(f.buffer).setUint32(0, b.length, true);
          f.set(b, 4);
          uw.write(f).catch(function () {});
          return 0;
        };
        n.close = function () {
          try {
            t.close();
          } catch (e) {}
        };
        (async function () {
          var r = t.datagrams.readable.getReader();
          for (;;) {
            var x = await r.read();
            if (x.done) break;
            n.q.push(x.value);
          }
        })().catch(function () {});
        (async function () {
          var us = t.incomingUnidirectionalStreams.getReader();
          for (;;) {
            var x = await us.read();
            if (x.done) break;
            readFramed(x.value).catch(function () {});
          }
        })().catch(function () {});
        t.closed.then(
          function () {
            n.state = 2;
          },
          function () {
            n.state = 2;
          },
        );
        n.kind = 2;
        n.state = 1;
      } catch (e) {
        try {
          if (t) t.close();
        } catch (e2) {}
        useWs();
      }
    }

    if (wtArg && typeof WebTransport !== "undefined") useWt();
    else useWs();
  },
  net_state: function () {
    return Module.__net ? Module.__net.state : 2;
  },
  net_kind: function () {
    return Module.__net ? Module.__net.kind : 0;
  },
  net_send: function (ch, ptr, len) {
    var n = Module.__net;
    if (!n || n.state !== 1) return -1;
    return n.send(ch, HEAPU8.slice(ptr, ptr + len));
  },
  net_poll: function (ptr, cap) {
    var n = Module.__net;
    if (!n || n.q.length === 0) return 0;
    var m = n.q.shift();
    if (m.length > cap) return -1;
    HEAPU8.set(m, ptr);
    return m.length;
  },
  net_close: function () {
    var n = Module.__net;
    if (n) {
      try {
        n.close();
      } catch (e) {}
    }
  },
  store_get__deps: ["$stringToUTF8", "$lengthBytesUTF8"],
  store_get: function (keyPtr, buf, cap) {
    var s = null;
    try {
      s = localStorage.getItem(UTF8ToString(keyPtr));
    } catch (e) {}
    if (s === null || lengthBytesUTF8(s) >= cap) return -1;
    return stringToUTF8(s, buf, cap);
  },
  store_set: function (keyPtr, valPtr) {
    try {
      localStorage.setItem(UTF8ToString(keyPtr), UTF8ToString(valPtr));
    } catch (e) {}
  },
});

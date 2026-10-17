mergeInto(LibraryManager.library, {
  wt_open__deps: ["$UTF8ToString"],
  wt_open: function (urlPtr) {
    const url = UTF8ToString(urlPtr);
    const MAX = 1024 * 1024;
    const S = (Module.wt = { state: 0, inbox: [], t: null, w: null, dg: null });
    const fail = () => {
      S.state = 2;
    };
    const push = (ch, data) => {
      if (data.length <= MAX) S.inbox.push([ch, data]);
    };

    async function readDatagrams(t) {
      const r = t.datagrams.readable.getReader();
      for (;;) {
        const { value, done } = await r.read();
        if (done) break;
        push(1, value);
      }
    }
    async function readStream(stream) {
      const r = stream.getReader();
      let buf = new Uint8Array(0);
      for (;;) {
        const { value, done } = await r.read();
        if (done) break;
        const nb = new Uint8Array(buf.length + value.length);
        nb.set(buf);
        nb.set(value, buf.length);
        buf = nb;
        while (buf.length >= 4) {
          const len = new DataView(buf.buffer, buf.byteOffset).getUint32(
            0,
            true,
          );
          if (len > MAX) throw new Error("packet too large");
          if (buf.length < 4 + len) break;
          push(0, buf.slice(4, 4 + len));
          buf = buf.slice(4 + len);
        }
      }
    }
    async function acceptStreams(t) {
      const r = t.incomingUnidirectionalStreams.getReader();
      for (;;) {
        const { value, done } = await r.read();
        if (done) break;
        readStream(value).catch(fail);
      }
    }

    try {
      const t = new WebTransport(url);
      S.t = t;
      t.closed.then(fail, fail);
      t.ready
        .then(() => {
          S.dg = t.datagrams.writable.getWriter();
          S.state = 1;
          readDatagrams(t).catch(fail);
          acceptStreams(t).catch(fail);

          S.chain = t.createUnidirectionalStream().then((s) => {
            S.w = s.getWriter();
          });
          S.chain.catch(fail);
        })
        .catch(fail);
    } catch (e) {
      console.error("wt fail", e);
      fail();
    }
  },

  wt_state: function () {
    return Module.wt ? Module.wt.state : 2;
  },

  wt_max_datagram_size: function () {
    const S = Module.wt;
    return S && S.t ? S.t.datagrams.maxDatagramSize : 0;
  },

  wt_send: function (ch, ptr, len) {
    const S = Module.wt;
    if (!S || S.state !== 1) return -1;
    if (ch === 0) {
      const f = new Uint8Array(4 + len);
      new DataView(f.buffer).setUint32(0, len, true);
      f.set(HEAPU8.subarray(ptr, ptr + len), 4);
      S.w.write(f).catch(function () {
        S.state = 2;
      });
    } else {
      S.dg.write(HEAPU8.slice(ptr, ptr + len)).catch(function () {});
    }
    return 0;
  },

  wt_poll: function (bufPtr, cap) {
    const S = Module.wt;
    if (!S || S.inbox.length === 0) return 0;
    const item = S.inbox.shift();
    if (1 + item[1].length > cap) return 0;
    HEAPU8[bufPtr] = item[0];
    HEAPU8.set(item[1], bufPtr + 1);
    return 1 + item[1].length;
  },

  wt_close: function () {
    const S = Module.wt;
    if (S && S.t) S.t.close();
  },
});

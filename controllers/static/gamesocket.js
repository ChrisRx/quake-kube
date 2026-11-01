// Game traffic for the browser clients. Load after defining `base`, the server's
// path on the gateway (/play/<namespace>/<name>), and `webTransport`, which says
// where the gateway accepts WebTransport sessions, or null when it doesn't.

let useWebTransport = Boolean(webTransport && window.WebTransport);

const NativeWebSocket = window.WebSocket;
const relayURL = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}${base}/ws`;

// Stands in for the WebSocket the client opens to reach the server. The client always
// connects to ws(s)://<host>:<port> with no path, so this connects to this server's relay
// on the gateway instead. It prefers WebTransport, whose unreliable datagrams behave like
// the UDP the game was built for, and falls back to a WebSocket. Only the parts of the
// WebSocket API the client uses are implemented.
class GameSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  CONNECTING = 0;
  OPEN = 1;
  CLOSING = 2;
  CLOSED = 3;

  constructor(url, protocols) {
    this.binaryType = "arraybuffer";
    this.readyState = GameSocket.CONNECTING;
    this.onopen = this.onmessage = this.onerror = this.onclose = null;
    this.connect(protocols);
  }

  async connect(protocols) {
    if (useWebTransport) {
      try {
        await this.connectWebTransport();
        console.info("Connected over WebTransport");
        this.opened();
        return;
      } catch (error) {
        // Don't wait on it again for later connections, e.g. when UDP is blocked.
        useWebTransport = false;
        console.warn("WebTransport unavailable, using a WebSocket:", error);
      }
    }
    this.connectWebSocket(protocols);
  }

  async connectWebTransport() {
    const host = webTransport.address || `${location.hostname}:${webTransport.port}`;
    const options = {};
    if (webTransport.certificateHashes.length) {
      // The gateway's certificate is self-signed, so it's trusted by its hash.
      options.serverCertificateHashes = webTransport.certificateHashes.map((hash) => ({
        algorithm: "sha-256",
        value: new Uint8Array(hash),
      }));
    }
    // Sessions don't go through the proxy that signs people in, so they bring a ticket
    // from the play page instead.
    const query = webTransport.ticket ? `?ticket=${encodeURIComponent(webTransport.ticket)}` : "";
    const transport = new WebTransport(`https://${host}${base}${query}`, options);
    const timeout = new Promise((_, reject) =>
      setTimeout(() => reject(new Error("timed out")), 3000),
    );
    try {
      await Promise.race([transport.ready, timeout]);
    } catch (error) {
      transport.close();
      throw error;
    }
    // Packets too large for a datagram go over this stream, each with a 2-byte length.
    const stream = await transport.createBidirectionalStream();
    this.transport = transport;
    this.datagrams = transport.datagrams.writable.getWriter();
    this.stream = stream.writable.getWriter();
    this.readDatagrams(transport.datagrams.readable.getReader());
    this.readStream(stream.readable.getReader());
    transport.closed.then(
      () => this.closed(),
      () => this.closed(),
    );
  }

  connectWebSocket(protocols) {
    this.ws = new NativeWebSocket(relayURL, protocols);
    this.ws.binaryType = "arraybuffer";
    this.ws.onopen = () => this.opened();
    this.ws.onmessage = (event) => this.received(event.data);
    this.ws.onerror = (event) => this.onerror?.(event);
    this.ws.onclose = () => this.closed();
  }

  opened() {
    // The client gave up on this connection before it opened.
    if (this.readyState === GameSocket.CLOSING) {
      this.close();
      return;
    }
    this.readyState = GameSocket.OPEN;
    this.onopen?.();
  }

  received(data) {
    this.onmessage?.({ data });
  }

  closed() {
    if (this.readyState === GameSocket.CLOSED) return;
    this.readyState = GameSocket.CLOSED;
    this.onclose?.();
  }

  async readDatagrams(reader) {
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        this.received(value.slice().buffer);
      }
    } catch {}
  }

  async readStream(reader) {
    let buffered = new Uint8Array(0);
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        const joined = new Uint8Array(buffered.length + value.length);
        joined.set(buffered);
        joined.set(value, buffered.length);
        buffered = joined;
        while (buffered.length >= 2) {
          const length = (buffered[0] << 8) | buffered[1];
          if (buffered.length < 2 + length) break;
          this.received(buffered.slice(2, 2 + length).buffer);
          buffered = buffered.subarray(2 + length);
        }
      }
    } catch {}
  }

  send(data) {
    if (this.ws) {
      this.ws.send(data);
      return;
    }
    const packet = new Uint8Array(data);
    const maxDatagramSize = this.transport.datagrams.maxDatagramSize ?? 1024;
    if (packet.length <= maxDatagramSize) {
      this.datagrams.write(packet).catch(() => {});
    } else {
      const framed = new Uint8Array(2 + packet.length);
      framed[0] = packet.length >> 8;
      framed[1] = packet.length & 0xff;
      framed.set(packet, 2);
      this.stream.write(framed).catch(() => {});
    }
  }

  close() {
    this.readyState = GameSocket.CLOSING;
    if (this.ws) {
      this.ws.close();
    } else {
      this.transport?.close();
    }
  }
}
window.WebSocket = GameSocket;

// SPDX-License-Identifier: Apache-2.0
(function (global) {
  const DEFAULT_PORT = 47825;
  const RECONNECT_MS = 3000;
  const API_VERSION = "^0.1";

  const Value = {
    bool: (value) => ({ type: "bool", value: !!value }),
    int: (value) => ({ type: "int", value: Math.trunc(value) }),
    float: (value) => ({ type: "float", value: value }),
    ratio: (value) => ({
      type: "ratio",
      value: Math.min(1, Math.max(0, value)),
    }),
    text: (value) => ({ type: "text", value: String(value) }),
    enumeration: (value) => ({ type: "enum", value: String(value) }),
  };

  const Outcome = {
    succeeded: () => ({ outcome: "succeeded" }),
    failed: (reason) => ({ outcome: "failed", reason: String(reason) }),
    rejected: (reason) => ({ outcome: "rejected", reason: String(reason) }),
    aborted: () => ({ outcome: "aborted" }),
  };

  function signal(id, value) {
    return { id, value };
  }

  function answer(request, mod) {
    switch (request.request) {
      case "hello":
        return {
          response: "hello",
          plugin: mod.plugin,
          api_version: mod.apiVersion || API_VERSION,
        };
      case "observe":
        return { response: "observed", signals: mod.observe() };
      case "act":
        return { response: "acted", outcome: mod.act(request.intent) };
      default:
        return {
          response: "error",
          message: `unknown request \`${request.request}\``,
        };
    }
  }

  function handle(raw, mod) {
    let request;
    try {
      request = JSON.parse(raw);
    } catch (error) {
      return { response: "error", message: `malformed request: ${error}` };
    }
    try {
      return answer(request, mod);
    } catch (error) {
      return {
        response: "error",
        message: String(error && error.message) || "the mod failed",
      };
    }
  }

  function serve(mod) {
    const log = mod.log || (() => {});
    const port = mod.port || DEFAULT_PORT;
    const url = `ws://127.0.0.1:${port}/${mod.endpoint}`;
    const open =
      mod.socketFactory || ((target) => new global.WebSocket(target));

    let socket = null;
    let timer = null;
    let stopped = false;

    function connect() {
      if (stopped) {
        return;
      }
      try {
        socket = open(url);
      } catch (error) {
        return retry();
      }

      socket.onmessage = (event) => {
        socket.send(JSON.stringify(handle(event.data, mod)));
      };
      socket.onopen = () => log(`bridge connected on ${url}`);
      socket.onerror = () => {};
      socket.onclose = () => {
        socket = null;
        retry();
      };
    }

    function retry() {
      if (stopped || timer !== null) {
        return;
      }
      timer = global.setTimeout(() => {
        timer = null;
        connect();
      }, mod.reconnectMs || RECONNECT_MS);
    }

    connect();

    return {
      stop() {
        stopped = true;
        if (timer !== null) {
          global.clearTimeout(timer);
          timer = null;
        }
        if (socket !== null) {
          socket.close();
          socket = null;
        }
      },
    };
  }

  global.IdleWardenBridge = {
    serve,
    handle,
    signal,
    Value,
    Outcome,
    DEFAULT_PORT,
    API_VERSION,
  };
})(typeof globalThis === "undefined" ? this : globalThis);

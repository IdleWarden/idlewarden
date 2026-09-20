// SPDX-License-Identifier: Apache-2.0
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

async function load(...files) {
  const scope = { setTimeout, clearTimeout };
  for (const file of files) {
    const source = await readFile(join(here, "..", file), "utf8");
    new Function("globalThis", `${source}`).call(scope, scope);
  }
  return scope;
}

function fakeSocket() {
  const sent = [];
  const socket = {
    sent,
    readyState: 1,
    send: (payload) => sent.push(JSON.parse(payload)),
    close: () => socket.onclose && socket.onclose(),
    receive: (message) => socket.onmessage({ data: JSON.stringify(message) }),
  };
  return socket;
}

const holding = (name, amount, price, storedCps, locked = false) => ({
  name,
  amount,
  storedCps,
  locked,
  getPrice: () => price,
  buy(count) {
    this.amount += count;
  },
});

const upgrade = (name, price, affordable = true) => ({
  name,
  bought: false,
  getPrice: () => price,
  canBuy: () => affordable,
  buy() {
    this.bought = true;
  },
});

const cookieClicker = () => ({
  cookies: 1200.7,
  cookiesPs: 42.5,
  globalCpsMult: 2,
  shimmers: [],
  UpgradesInStore: [],
  UpgradesOwned: 41,
  Objects: {
    Cursor: holding("Cursor", 7, 900, 0.1),
  },
  ClickCookie() {
    this.cookies += 1;
  },
  Notify: () => {},
});

test("the handshake names the plugin and the protocol the mod was built against", async () => {
  const { IdleWardenBridge } = await load("idlewarden-bridge/bridge.js");

  const hello = IdleWardenBridge.handle(JSON.stringify({ request: "hello" }), {
    plugin: "dev.example.game",
    observe: () => [],
    act: () => ({}),
  });

  assert.deepEqual(hello, {
    response: "hello",
    plugin: "dev.example.game",
    api_version: "^0.1",
  });
});

test("a value carries its type the way the host reads it", async () => {
  const { IdleWardenBridge } = await load("idlewarden-bridge/bridge.js");
  const { Value } = IdleWardenBridge;

  assert.deepEqual(Value.int(12.9), { type: "int", value: 12 });
  assert.deepEqual(Value.ratio(1.4), { type: "ratio", value: 1 });
  assert.deepEqual(Value.ratio(-3), { type: "ratio", value: 0 });
  assert.deepEqual(Value.enumeration("main"), { type: "enum", value: "main" });
});

test("a mod that throws answers with an error rather than dropping the connection", async () => {
  const { IdleWardenBridge } = await load("idlewarden-bridge/bridge.js");

  const answer = IdleWardenBridge.handle(
    JSON.stringify({ request: "observe" }),
    {
      plugin: "dev.example.game",
      observe: () => {
        throw new Error("the save is still loading");
      },
      act: () => ({}),
    },
  );

  assert.equal(answer.response, "error");
  assert.match(answer.message, /still loading/);
});

test("a request that is not JSON is answered, not swallowed", async () => {
  const { IdleWardenBridge } = await load("idlewarden-bridge/bridge.js");

  const answer = IdleWardenBridge.handle("{ not json", {
    plugin: "dev.example.game",
    observe: () => [],
    act: () => ({}),
  });

  assert.equal(answer.response, "error");
});

test("the socket answers every request the host sends", async () => {
  const scope = await load("idlewarden-bridge/bridge.js");
  const socket = fakeSocket();

  scope.IdleWardenBridge.serve({
    endpoint: "cookie-clicker",
    plugin: "dev.example.game",
    observe: () => [
      scope.IdleWardenBridge.signal("resource.cookies", {
        type: "int",
        value: 3,
      }),
    ],
    act: () => scope.IdleWardenBridge.Outcome.succeeded(),
    socketFactory: () => socket,
  });

  socket.receive({ request: "hello" });
  socket.receive({ request: "observe" });
  socket.receive({
    request: "act",
    intent: { name: "click_cookie", params: {} },
  });

  assert.deepEqual(
    socket.sent.map((answer) => answer.response),
    ["hello", "observed", "acted"],
  );
  assert.deepEqual(socket.sent[1].signals[0].id, "resource.cookies");
  assert.deepEqual(socket.sent[2].outcome, { outcome: "succeeded" });
});

test("losing the host is not fatal: the mod waits and connects again", async () => {
  const scope = await load("idlewarden-bridge/bridge.js");
  const sockets = [];

  scope.IdleWardenBridge.serve({
    endpoint: "cookie-clicker",
    plugin: "dev.example.game",
    observe: () => [],
    act: () => ({}),
    reconnectMs: 1,
    socketFactory: () => {
      const socket = fakeSocket();
      sockets.push(socket);
      return socket;
    },
  });

  sockets[0].close();
  await new Promise((resolve) => setTimeout(resolve, 20));

  assert.equal(
    sockets.length,
    2,
    "a session that starts later has to find the mod again",
  );
});

test("the endpoint is in the url, so one game's mod cannot answer for another", async () => {
  const scope = await load("idlewarden-bridge/bridge.js");
  let opened = "";

  scope.IdleWardenBridge.serve({
    endpoint: "cookie-clicker",
    plugin: "dev.example.game",
    observe: () => [],
    act: () => ({}),
    socketFactory: (url) => {
      opened = url;
      return fakeSocket();
    },
  });

  assert.equal(opened, "ws://127.0.0.1:47825/cookie-clicker");
});

test("the golden cookie is reported and popping it is carried out by the game", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  scope.Game = game;
  const { observe, act } = scope.IdleWardenCookieClicker;

  const before = observe().find((signal) => signal.id === "ui.golden_cookie");
  assert.deepEqual(before.value, { type: "bool", value: false });
  assert.deepEqual(act({ name: "pop_golden_cookie", params: {} }), {
    outcome: "failed",
    reason: "no golden cookie is on screen",
  });

  let popped = false;
  game.shimmers.push({ type: "golden", pop: () => (popped = true) });

  const after = observe().find((signal) => signal.id === "ui.golden_cookie");
  assert.deepEqual(after.value, { type: "bool", value: true });
  assert.deepEqual(act({ name: "pop_golden_cookie", params: {} }), {
    outcome: "succeeded",
  });
  assert.equal(popped, true);
});

test("what the mod reports is what the game holds, not a reading of the screen", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  scope.Game = cookieClicker();

  const signals = Object.fromEntries(
    scope.IdleWardenCookieClicker.observe().map((signal) => [
      signal.id,
      signal.value,
    ]),
  );

  assert.deepEqual(signals["resource.cookies"], { type: "int", value: 1200 });
  assert.deepEqual(signals["rate.cookies_per_second"], {
    type: "float",
    value: 42.5,
  });
  assert.deepEqual(signals["stat.cursors"], { type: "int", value: 7 });
});

test("an intent the mod does not know is rejected, and one it cannot afford fails", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  scope.Game = game;
  const { act } = scope.IdleWardenCookieClicker;

  assert.deepEqual(act({ name: "ascend", params: {} }), {
    outcome: "rejected",
    reason: "unknown intent `ascend`",
  });
  assert.deepEqual(act({ name: "buy_building", params: {} }), {
    outcome: "rejected",
    reason: "buy_building needs a `building` parameter",
  });

  game.cookies = 10;
  const outcome = act({
    name: "buy_building",
    params: { building: { type: "text", value: "Cursor" } },
  });
  assert.equal(outcome.outcome, "failed");
  assert.equal(
    game.Objects.Cursor.amount,
    7,
    "a failed purchase must not have bought anything",
  );

  game.cookies = 5000;
  assert.deepEqual(
    act({
      name: "buy_building",
      params: { building: { type: "text", value: "Cursor" } },
    }),
    { outcome: "succeeded" },
  );
  assert.equal(game.Objects.Cursor.amount, 8);
});

test("the mod buys the cheapest upgrade it can afford, not the first listed", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  const dear = upgrade("Dear", 5000);
  const cheap = upgrade("Cheap", 300);
  const unaffordable = upgrade("Unaffordable", 10, false);
  game.UpgradesInStore = [dear, unaffordable, cheap];
  scope.Game = game;

  assert.deepEqual(
    scope.IdleWardenCookieClicker.act({ name: "buy_upgrade", params: {} }),
    {
      outcome: "succeeded",
    },
  );

  assert.equal(cheap.bought, true);
  assert.equal(dear.bought, false);
  assert.equal(
    unaffordable.bought,
    false,
    "canBuy is the game's own answer, and it said no",
  );
});

test("with nothing affordable in the store, buying an upgrade fails", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  game.UpgradesInStore = [upgrade("Dear", 5000, false)];
  scope.Game = game;

  const outcome = scope.IdleWardenCookieClicker.act({
    name: "buy_upgrade",
    params: {},
  });

  assert.equal(outcome.outcome, "failed");
});

test("the best building is the one that pays for itself soonest", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  game.Objects = {
    Cursor: holding("Cursor", 7, 900, 0.1),
    Grandma: holding("Grandma", 3, 1000, 5),
    Farm: holding("Farm", 0, 100000, 10),
    Locked: holding("Locked", 0, 1, 1000, true),
  };
  scope.Game = game;

  const signals = Object.fromEntries(
    scope.IdleWardenCookieClicker.observe().map((signal) => [
      signal.id,
      signal.value,
    ]),
  );

  assert.deepEqual(signals["building.best_payback"], {
    type: "enum",
    value: "Grandma",
  });
  assert.deepEqual(signals["building.best_payback_affordable"], {
    type: "bool",
    value: true,
  });
  assert.deepEqual(signals["stat.buildings_owned"], { type: "int", value: 10 });
  assert.deepEqual(signals["stat.upgrades_owned"], { type: "int", value: 41 });
});

test("a building that earns nothing is never the best buy", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  game.Objects = { Cursor: holding("Cursor", 0, 15, 0) };
  scope.Game = game;

  const signals = Object.fromEntries(
    scope.IdleWardenCookieClicker.observe().map((signal) => [
      signal.id,
      signal.value,
    ]),
  );

  assert.deepEqual(signals["building.best_payback"], {
    type: "enum",
    value: "none",
  });
  assert.deepEqual(
    scope.IdleWardenCookieClicker.act({
      name: "buy_best_building",
      params: {},
    }),
    {
      outcome: "failed",
      reason: "no building earns anything yet",
    },
  );
});

test("buying the best building buys that one, and fails rather than overdrawing", async () => {
  const scope = await load(
    "idlewarden-bridge/bridge.js",
    "cookie-clicker/mod.js",
  );
  const game = cookieClicker();
  const grandma = holding("Grandma", 3, 1000, 5);
  game.Objects = { Cursor: holding("Cursor", 7, 900, 0.1), Grandma: grandma };
  scope.Game = game;

  assert.deepEqual(
    scope.IdleWardenCookieClicker.act({
      name: "buy_best_building",
      params: {},
    }),
    { outcome: "succeeded" },
  );
  assert.equal(grandma.amount, 4);

  game.cookies = 10;
  const outcome = scope.IdleWardenCookieClicker.act({
    name: "buy_best_building",
    params: {},
  });
  assert.equal(outcome.outcome, "failed");
  assert.equal(
    grandma.amount,
    4,
    "a failed purchase must not have bought anything",
  );
});

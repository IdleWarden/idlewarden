// SPDX-License-Identifier: MPL-2.0
(function (global) {
  const bridge = global.IdleWardenBridge;
  const ENDPOINT = "cookie-clicker";
  const PLUGIN = "dev.idlewarden.cookie-clicker";
  const MOD_ID = "idlewarden bridge";

  function golden() {
    const shimmers = global.Game.shimmers || [];
    return shimmers.find((shimmer) => shimmer.type === "golden") || null;
  }

  function affordable() {
    const store = global.Game.UpgradesInStore || [];
    return store.find((upgrade) => upgrade.canBuy()) || null;
  }

  function building(name) {
    const objects = global.Game.Objects || {};
    return Object.prototype.hasOwnProperty.call(objects, name)
      ? objects[name]
      : null;
  }

  function parameter(intent, key) {
    const params = (intent && intent.params) || {};
    return Object.prototype.hasOwnProperty.call(params, key)
      ? params[key].value
      : null;
  }

  function observe() {
    const Game = global.Game;
    const cursors = building("Cursor");
    return [
      bridge.signal("resource.cookies", bridge.Value.int(Game.cookies)),
      bridge.signal(
        "rate.cookies_per_second",
        bridge.Value.float(Game.cookiesPs),
      ),
      bridge.signal("ui.golden_cookie", bridge.Value.bool(golden() !== null)),
      bridge.signal(
        "upgrade.affordable",
        bridge.Value.bool(affordable() !== null),
      ),
      bridge.signal(
        "stat.cursors",
        bridge.Value.int(cursors ? cursors.amount : 0),
      ),
    ];
  }

  function act(intent) {
    switch (intent.name) {
      case "click_cookie":
        global.Game.ClickCookie();
        return bridge.Outcome.succeeded();

      case "pop_golden_cookie": {
        const shimmer = golden();
        if (shimmer === null) {
          return bridge.Outcome.failed("no golden cookie is on screen");
        }
        shimmer.pop();
        return bridge.Outcome.succeeded();
      }

      case "buy_upgrade": {
        const upgrade = affordable();
        if (upgrade === null) {
          return bridge.Outcome.failed("no upgrade in the store is affordable");
        }
        upgrade.buy();
        return bridge.Outcome.succeeded();
      }

      case "buy_building": {
        const name = parameter(intent, "building");
        if (name === null) {
          return bridge.Outcome.rejected(
            "buy_building needs a `building` parameter",
          );
        }
        const object = building(name);
        if (object === null) {
          return bridge.Outcome.rejected(`this game has no \`${name}\``);
        }
        if (object.getPrice() > global.Game.cookies) {
          return bridge.Outcome.failed(
            `not affordable: ${name} costs ${object.getPrice()}`,
          );
        }
        object.buy(1);
        return bridge.Outcome.succeeded();
      }

      default:
        return bridge.Outcome.rejected(`unknown intent \`${intent.name}\``);
    }
  }

  global.IdleWardenCookieClicker = { observe, act, ENDPOINT, PLUGIN };

  if (global.Game && global.Game.registerMod) {
    global.Game.registerMod(MOD_ID, {
      init: function () {
        bridge.serve({
          endpoint: ENDPOINT,
          plugin: PLUGIN,
          observe: observe,
          act: act,
          log: (message) => global.Game.Notify("IdleWarden", message, [16, 5]),
        });
      },
      save: function () {},
      load: function () {},
    });
  }
})(typeof globalThis === "undefined" ? this : globalThis);

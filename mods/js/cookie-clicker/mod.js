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

  function cheapestUpgrade() {
    const store = global.Game.UpgradesInStore || [];
    return (
      store
        .filter((upgrade) => upgrade.canBuy())
        .sort((a, b) => a.getPrice() - b.getPrice())[0] || null
    );
  }

  function paybackSeconds(object) {
    const earns = (object.storedCps || 0) * (global.Game.globalCpsMult || 1);
    return earns > 0 ? object.getPrice() / earns : Infinity;
  }

  function bestPayback() {
    const objects = Object.values(global.Game.Objects || {});
    let best = null;
    for (const object of objects) {
      if (object.locked) {
        continue;
      }
      if (best === null || paybackSeconds(object) < paybackSeconds(best)) {
        best = object;
      }
    }
    return best !== null && paybackSeconds(best) < Infinity ? best : null;
  }

  function owned() {
    return Object.values(global.Game.Objects || {}).reduce(
      (total, object) => total + (object.amount || 0),
      0,
    );
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
    const best = bestPayback();
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
      bridge.signal(
        "stat.upgrades_owned",
        bridge.Value.int(Game.UpgradesOwned || 0),
      ),
      bridge.signal("stat.buildings_owned", bridge.Value.int(owned())),
      bridge.signal(
        "building.best_payback",
        bridge.Value.enumeration(best === null ? "none" : best.name),
      ),
      bridge.signal(
        "building.best_payback_affordable",
        bridge.Value.bool(best !== null && best.getPrice() <= Game.cookies),
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
        const upgrade = cheapestUpgrade();
        if (upgrade === null) {
          return bridge.Outcome.failed("no upgrade in the store is affordable");
        }
        upgrade.buy();
        return bridge.Outcome.succeeded();
      }

      case "buy_best_building": {
        const best = bestPayback();
        if (best === null) {
          return bridge.Outcome.failed("no building earns anything yet");
        }
        if (best.getPrice() > global.Game.cookies) {
          return bridge.Outcome.failed(
            `not affordable: ${best.name} costs ${best.getPrice()}`,
          );
        }
        best.buy(1);
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

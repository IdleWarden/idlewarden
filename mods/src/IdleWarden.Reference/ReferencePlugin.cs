// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using BepInEx;
using IdleWarden.Bridge;

namespace IdleWarden.Reference
{
    /// <summary>
    /// The smallest mod that speaks the bridge protocol correctly.
    /// <para>
    /// It exposes a synthetic counter rather than a real game's state, so it can
    /// be dropped into any Unity title to prove the pipe end to end. Replace
    /// <see cref="ReferenceBridge"/> with reads of the game's own fields; the
    /// plumbing below does not change.
    /// </para>
    /// </summary>
    [BepInPlugin(PluginId, "IdleWarden reference bridge", "0.0.0")]
    public sealed class ReferencePlugin : BaseUnityPlugin
    {
        private const string PluginId = "com.idlewarden.reference";
        private const string Endpoint = "reference";

        private BridgeServer server;

        private void Awake()
        {
            server = new BridgeServer(Endpoint, new ReferenceBridge(), message => Logger.LogInfo(message));
            server.Start();
        }

        private void Update()
        {
            server?.Pump();
        }

        private void OnDestroy()
        {
            server?.Dispose();
            server = null;
        }
    }

    internal sealed class ReferenceBridge : IGameBridge
    {
        private long cookies;
        private long clicks;

        public string PluginId => "dev.idlewarden.reference";

        public string ApiVersion => "^0.1";

        public IReadOnlyList<Signal> Observe()
        {
            cookies += 1;

            return new List<Signal>
            {
                new Signal("ui.screen_id", Value.Enum("main")),
                new Signal("resource.cookies", Value.Int(cookies)),
                new Signal("stat.clicks", Value.Int(clicks)),
                new Signal("ui.progress", Value.Ratio((cookies % 100) / 100.0)),
            };
        }

        public ActionOutcome Act(Intent intent)
        {
            switch (intent.Name)
            {
                case "click":
                    clicks += 1;
                    return ActionOutcome.Succeeded;

                case "buy_upgrade":
                    var tier = intent.Parameter("tier");
                    if (tier == null)
                    {
                        return ActionOutcome.Rejected("buy_upgrade needs a `tier` parameter");
                    }

                    var price = tier.AsInt() * 100;
                    if (cookies < price)
                    {
                        return ActionOutcome.Failed("not affordable: " + cookies + " of " + price);
                    }

                    cookies -= price;
                    return ActionOutcome.Succeeded;

                default:
                    return ActionOutcome.Rejected("unknown intent `" + intent.Name + "`");
            }
        }
    }
}

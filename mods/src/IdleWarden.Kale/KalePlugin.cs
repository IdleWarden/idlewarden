// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using BepInEx;
using IdleWarden.Bridge;
using IdleWarden.Kale.Decisions;

namespace IdleWarden.Kale
{
    [BepInPlugin(PluginId, MyPluginInfo.PLUGIN_NAME, MyPluginInfo.PLUGIN_VERSION)]
    public sealed class KalePlugin : BaseUnityPlugin
    {
        private const string PluginId = "com.idlewarden.kale";
        private const string Endpoint = "master-healer-kale";

        private BridgeServer server;

        private void Awake()
        {
            server = new BridgeServer(Endpoint, new KaleBridge(), message => Logger.LogInfo(message));
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

    internal sealed class KaleBridge : IGameBridge
    {
        /// Below this share of maximum health a unit is worth a targeted spell.
        /// It only decides which unit a cast lands on; when to cast at all is the
        /// plugin's rules, which read `party.lowest_hp_ratio` and choose.
        private const double Hurt = 0.999;

        public string PluginId => "dev.idlewarden.master-healer-kale";

        public string ApiVersion => "^0.1";

        public IReadOnlyList<Signal> Observe()
        {
            return KaleSignals.From(GameReader.Read());
        }

        public ActionOutcome Act(Intent intent)
        {
            switch (intent.Name)
            {
                case "cast":
                    return Cast(intent);

                case "buy_cheapest_skill":
                    GameReader.RefreshTree(force: true);
                    return GameActions.Buy(
                        SkillChoice.Cheapest(GameReader.Read().Nodes),
                        "in the skill tree");

                case "buy_best_skill":
                    return BuyBest(intent);

                default:
                    return ActionOutcome.Rejected("unknown intent `" + intent.Name + "`");
            }
        }

        private static ActionOutcome Cast(Intent intent)
        {
            var spell = intent.Parameter("spell");
            if (spell == null)
            {
                return ActionOutcome.Rejected("cast needs a `spell` parameter naming one of the game's spells");
            }

            var asked = intent.Parameter("target")?.AsString();
            if (!CastPlan.TryPolicy(asked, out var policy))
            {
                return ActionOutcome.Rejected(
                    "`" + asked + "` is not a target policy; use `lowest_hp`, `down` or `none`");
            }

            return GameActions.Cast(GameReader.Read(), spell.AsString(), policy, Hurt);
        }

        private static ActionOutcome BuyBest(Intent intent)
        {
            var currency = intent.Parameter("currency")?.AsString();
            var kind = intent.Parameter("kind")?.AsString();
            if (currency == null || kind == null)
            {
                return ActionOutcome.Rejected(
                    "buy_best_skill needs `currency` and `kind`, because effect per cost only compares"
                    + " within one currency and one kind of node");
            }

            GameReader.RefreshTree(force: true);
            return GameActions.Buy(
                SkillChoice.BestGainPerCost(GameReader.Read().Nodes, currency, kind),
                "paid in " + currency + " among " + kind + " nodes");
        }
    }
}

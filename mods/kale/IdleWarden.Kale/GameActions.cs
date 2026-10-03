// SPDX-License-Identifier: MPL-2.0

using System;
using System.Linq;
using IdleWarden.Bridge;
using IdleWarden.Kale.Decisions;

namespace IdleWarden.Kale
{
    internal static class GameActions
    {
        /// Casting is queued and takes time, so the heal itself cannot be seen
        /// from here. What is observed before reporting success is the queue: the
        /// spell was not in it and now is. Whether the health actually came back
        /// is the host's post-condition, which compares this observation with the
        /// next one (ADR-0003).
        internal static ActionOutcome Cast(KaleSnapshot state, string spellName, TargetPolicy policy, double healBelow)
        {
            var plan = CastPlan.For(state, spellName, policy, healBelow);
            if (!plan.Allowed)
            {
                var why = CastGate.Explain(plan.Refusal, spellName);
                return plan.Refusal == CastRefusal.NoSpell
                    ? ActionOutcome.Rejected(why)
                    : ActionOutcome.Failed(why);
            }

            var library = LibraryActiveAbility.SharedLibraryActiveAbility;
            var kale = PrefabHeroKale.SharedPrefabHeroKale;
            if (library == null || kale == null)
            {
                return ActionOutcome.Failed("no battle is running");
            }

            var ability = library.GetActiveAbilityByName(spellName);
            if (ability == null)
            {
                return ActionOutcome.Rejected("`" + spellName + "` is not unlocked in this save");
            }

            PrefabUnit target = null;
            if (plan.NeedsTarget)
            {
                target = Find(plan.Target.Name);
                if (target == null)
                {
                    return ActionOutcome.Failed("`" + plan.Target.Name + "` left the battle before the cast");
                }
            }

            kale.StartCastingSpell(ability, target);

            return kale.HasQueueAbility(ability)
                ? ActionOutcome.Succeeded
                : ActionOutcome.Failed("the game refused `" + spellName + "` without saying why");
        }

        internal static ActionOutcome Buy(SkillCandidate chosen, string what)
        {
            if (GameReader.Scene() != GameReader.Tavern)
            {
                return ActionOutcome.Failed("the skill tree is only reachable from the tavern");
            }
            if (chosen == null)
            {
                return ActionOutcome.Failed("the game reports nothing takeable " + what);
            }

            var node = Node(chosen.Name);
            if (node == null)
            {
                return ActionOutcome.Failed("`" + chosen.Name + "` is no longer in the tree");
            }

            var before = node.GetSkillNodeLevel();
            node.FuncClick();
            var after = node.GetSkillNodeLevel();

            if (after > before)
            {
                return ActionOutcome.Succeeded;
            }

            // The click is swallowed rather than refused while the tutorial is
            // pointing somewhere else, which is why the level can sit still on a
            // node the tree had just called takeable.
            return ActionOutcome.Failed(
                "`" + chosen.Name + "` stayed at level " + before
                + "; the tutorial or a requirement is holding it");
        }

        internal static ActionOutcome BuyResearch(SkillCandidate chosen)
        {
            if (GameReader.Scene() != GameReader.Tavern)
            {
                return ActionOutcome.Failed("the research facility is only reachable from the tavern");
            }
            if (chosen == null)
            {
                return ActionOutcome.Failed("no research is affordable");
            }

            var row = GameReader.LiveResearch().FirstOrDefault(found => found.UpgradeIndex == chosen.Name);
            if (row == null)
            {
                return ActionOutcome.Failed("`" + chosen.Name + "` is not in the research facility");
            }

            var database = PlayerDatabase.SharedPlayerDatabase;
            return Upgrade(
                chosen.Name,
                () => database.GetIntSaveObject(GameReader.ResearchKey(row)),
                row.RefreshUI,
                row.GetPrice,
                database.GetPlayerGold,
                () => row.ButtonClick("ButtonUpgrade"));
        }

        internal static ActionOutcome BuyTraining(SkillCandidate chosen)
        {
            if (GameReader.Scene() != GameReader.Tavern)
            {
                return ActionOutcome.Failed("the training facility is only reachable from the tavern");
            }
            if (chosen == null)
            {
                return ActionOutcome.Failed("no training is affordable");
            }

            var row = GameReader.LiveTraining().FirstOrDefault(found => found.UpgradeIndex.ToString() == chosen.Name);
            if (row == null)
            {
                return ActionOutcome.Failed("`" + chosen.Name + "` is not in the training facility");
            }

            var database = PlayerDatabase.SharedPlayerDatabase;
            return Upgrade(
                chosen.Name,
                () => database.GetIntSaveObject(row.UpgradeIndex.ToString()),
                row.RefreshUI,
                row.GetPrice,
                database.GetPlayerSkillPoint,
                () => row.ButtonClick("ButtonUpgrade"));
        }

        private static ActionOutcome Upgrade(
            string name,
            Func<int> level,
            Action refresh,
            Func<double_evrac> price,
            Func<double_evrac> wallet,
            Action click)
        {
            refresh();
            if (wallet() < price())
            {
                return ActionOutcome.Failed("`" + name + "` costs more than is held");
            }

            var before = level();
            click();
            return level() > before
                ? ActionOutcome.Succeeded
                : ActionOutcome.Failed("`" + name + "` stayed at level " + before);
        }

        private static PrefabUnit Find(string name)
        {
            foreach (var unit in GameReader.LiveParty())
            {
                if (GameReader.Name(unit) == name)
                {
                    return unit;
                }
            }
            return null;
        }

        private static PrefabSkillTree Node(string name)
        {
            foreach (var node in GameReader.LiveNodes())
            {
                if (node.ThisSkillNode != null && node.ThisSkillNode.Name == name && node.CanLevelup)
                {
                    return node;
                }
            }
            return null;
        }
    }
}

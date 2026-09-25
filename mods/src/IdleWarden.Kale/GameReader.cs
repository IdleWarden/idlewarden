// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using IdleWarden.Kale.Decisions;
using UnityEngine;

namespace IdleWarden.Kale
{
    internal static class GameReader
    {
        internal const string Tavern = "TAVERN";

        private static float nextTreeRefresh;

        internal static KaleSnapshot Read()
        {
            var scene = Scene();
            var database = PlayerDatabase.SharedPlayerDatabase;
            if (database == null)
            {
                return Empty(scene);
            }

            var battle = EngineBattle.SharedEngineBattle;
            var fighting = battle != null && battle.Isplaying;

            return new KaleSnapshot(
                scene,
                database.GetIntSaveObject("CurrentTutorial"),
                Of(database.GetPlayerGold()),
                Of(database.GetPlayerSkillPoint()),
                Of(database.GetPlayerRuby()),
                database.GetHighestLevelClear(),
                Completion(),
                Nodes(scene),
                fighting,
                fighting ? battle.CurrentFloor : 0,
                Mana(),
                database.GetPlayerMaxMana(),
                Party(),
                Spells());
        }

        internal static string Scene()
        {
            var transition = PopupTransition.SharedPopupTransition;
            return transition == null ? "unknown" : transition.GetActiveSceneName();
        }

        internal static Magnitude Of(double_evrac value)
        {
            return new Magnitude(value._Value, value._ZeroCount);
        }

        /// The game recomputes every node's eligibility here, and it is the only
        /// way `CanLevelup` becomes true. It also queues a delayed refresh of its
        /// own, so this is throttled rather than run on every observation.
        internal static void RefreshTree(bool force)
        {
            var tavern = EngineTavern.SharedEngineTavern;
            if (tavern == null)
            {
                return;
            }
            if (!force && Time.realtimeSinceStartup < nextTreeRefresh)
            {
                return;
            }

            nextTreeRefresh = Time.realtimeSinceStartup + 1f;
            tavern.RefreshSkillTree();
        }

        internal static IReadOnlyList<PrefabSkillTree> LiveNodes()
        {
            return Object.FindObjectsByType<PrefabSkillTree>(
                FindObjectsInactive.Include,
                FindObjectsSortMode.None);
        }

        internal static IReadOnlyList<PrefabUnit> LiveParty()
        {
            return Object.FindObjectsByType<PrefabUnit>(
                FindObjectsInactive.Exclude,
                FindObjectsSortMode.None);
        }

        internal static SkillCandidate Describe(PrefabSkillTree node)
        {
            var skill = node.ThisSkillNode;
            if (skill == null)
            {
                return null;
            }

            var level = node.GetSkillNodeLevel();
            var cost = Cost(skill, level);
            return new SkillCandidate(
                skill.Name,
                skill.CurrencyUsed.ToString(),
                cost,
                node.CanLevelup,
                skill.NodeType.ToString(),
                Gain(skill, level));
        }

        private static Magnitude Cost(SkillNode skill, int level)
        {
            var next = level + 1;
            if (skill.Price == null || next >= skill.Price.Length)
            {
                return Magnitude.Zero;
            }
            return Of(skill.Price[next]);
        }

        private static Magnitude Gain(SkillNode skill, int level)
        {
            if (skill.SkillNumber == null || skill.SkillNumber.Length == 0)
            {
                return Magnitude.Zero;
            }
            return Of(skill.GetSkillNumber(level + 1) - skill.GetSkillNumber(level));
        }

        private static IReadOnlyList<SkillCandidate> Nodes(string scene)
        {
            if (scene != Tavern)
            {
                return null;
            }

            RefreshTree(force: false);
            var described = new List<SkillCandidate>();
            foreach (var node in LiveNodes())
            {
                var candidate = Describe(node);
                if (candidate != null)
                {
                    described.Add(candidate);
                }
            }
            return described;
        }

        private static IReadOnlyList<UnitSnapshot> Party()
        {
            var party = new List<UnitSnapshot>();
            foreach (var unit in LiveParty())
            {
                party.Add(new UnitSnapshot(
                    Name(unit),
                    Of(unit.CurrentHP),
                    Of(unit.GetMaxHP()),
                    unit.IsAlive()));
            }
            return party;
        }

        internal static string Name(PrefabUnit unit)
        {
            return unit.ThisUnitHero == null ? unit.name : unit.ThisUnitHero.GetName();
        }

        private static IReadOnlyList<SpellSnapshot> Spells()
        {
            var library = LibraryActiveAbility.SharedLibraryActiveAbility;
            var kale = PrefabHeroKale.SharedPrefabHeroKale;
            if (library == null)
            {
                return null;
            }

            var spells = new List<SpellSnapshot>();
            foreach (var ability in library.GetAllUnlockedAbilities())
            {
                if (ability == null)
                {
                    continue;
                }
                spells.Add(new SpellSnapshot(
                    ability.Name,
                    ability.GetManaCost(),
                    ability.CurrentCooldown,
                    ability.GetCastingTime(),
                    ability.CanTargetDown,
                    kale != null && kale.HasQueueAbility(ability)));
            }
            return spells;
        }

        private static int Mana()
        {
            var kale = PrefabHeroKale.SharedPrefabHeroKale;
            return kale == null ? 0 : kale.CurrentMana;
        }

        private static int Completion()
        {
            var tavern = EngineTavern.SharedEngineTavern;
            return tavern == null ? 0 : tavern.GetPercentageTavernCompletion();
        }

        private static KaleSnapshot Empty(string scene)
        {
            return new KaleSnapshot(
                scene, 0,
                Magnitude.Zero, Magnitude.Zero, Magnitude.Zero,
                0, 0, null,
                false, 0, 0, 0,
                null, null);
        }
    }
}

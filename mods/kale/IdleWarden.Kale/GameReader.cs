// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using System.Reflection;
using IdleWarden.Kale.Decisions;
using UnityEngine;

namespace IdleWarden.Kale
{
    internal static class GameReader
    {
        internal const string Tavern = "TAVERN";

        private static float nextTreeRefresh;

        private static readonly FieldInfo Animation = typeof(PrefabHeroKale).GetField(
            "CurrentAnimation", BindingFlags.Instance | BindingFlags.NonPublic);

        internal static KaleSnapshot Read()
        {
            var scene = Scene();
            var database = PlayerDatabase.SharedPlayerDatabase;
            if (database == null)
            {
                return Empty(scene);
            }

            var battle = EngineBattle.SharedEngineBattle;
            var playing = battle != null && battle.Isplaying;
            var fighting = playing && !Moving();

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
                playing ? battle.CurrentFloor : 0,
                Mana(),
                database.GetPlayerMaxMana(),
                Party(),
                Spells(),
                database.Playtime,
                Research(scene, database),
                Training(scene, database));
        }

        internal static bool Moving()
        {
            var kale = PrefabHeroKale.SharedPrefabHeroKale;
            return kale != null && Animation?.GetValue(kale)?.ToString() == "Push";
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

        // `FindObjectsByType` replaced this in Unity 2023, and the game runs
        // Unity 6, but the newest UnityEngine on NuGet is 2021 and that is what
        // CI compiles against. Using the newer call would mean CI could no longer
        // build this mod at all, which is the whole point of the stubs. The
        // deprecated overload is still there in Unity 6, so one source compiles
        // against both and ships as one binary.
#pragma warning disable CS0618

        /// Inactive ones included: the tree deactivates a node's children as it
        /// refreshes, and a node that is not drawn is still one the game will
        /// accept a click on.
        internal static IReadOnlyList<PrefabSkillTree> LiveNodes()
        {
            return Object.FindObjectsOfType<PrefabSkillTree>(true);
        }

        /// Inactive ones excluded here, because a party member that is not in the
        /// scene is not in the fight.
        internal static IReadOnlyList<PrefabUnit> LiveParty()
        {
            return Object.FindObjectsOfType<PrefabUnit>(false);
        }

        internal static IReadOnlyList<PrefabResearchFacility> LiveResearch()
        {
            return Object.FindObjectsOfType<PrefabResearchFacility>(true);
        }

        internal static IReadOnlyList<PrefabTrainingFacility> LiveTraining()
        {
            return Object.FindObjectsOfType<PrefabTrainingFacility>(true);
        }

#pragma warning restore CS0618

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
                Gain(skill, level),
                level);
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

        internal static string ResearchKey(PrefabResearchFacility row)
        {
            return "Research_" + row.UpgradeIndex;
        }

        private static IReadOnlyList<SkillCandidate> Research(string scene, PlayerDatabase database)
        {
            if (scene != Tavern)
            {
                return null;
            }

            var gold = database.GetPlayerGold();
            var rows = new List<SkillCandidate>();
            foreach (var row in LiveResearch())
            {
                if (string.IsNullOrEmpty(row.UpgradeIndex))
                {
                    continue;
                }
                var price = row.GetPrice();
                rows.Add(new SkillCandidate(
                    row.UpgradeIndex,
                    "Gold",
                    Of(price),
                    !(gold < price),
                    "Research",
                    Magnitude.Zero,
                    database.GetIntSaveObject(ResearchKey(row))));
            }
            return rows;
        }

        private static IReadOnlyList<SkillCandidate> Training(string scene, PlayerDatabase database)
        {
            if (scene != Tavern)
            {
                return null;
            }

            var points = database.GetPlayerSkillPoint();
            var rows = new List<SkillCandidate>();
            foreach (var row in LiveTraining())
            {
                var price = row.GetPrice();
                rows.Add(new SkillCandidate(
                    row.UpgradeIndex.ToString(),
                    "SkillPoint",
                    Of(price),
                    !(points < price),
                    "Training",
                    Magnitude.Zero,
                    database.GetIntSaveObject(row.UpgradeIndex.ToString())));
            }
            return rows;
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
                    unit.IsAlive(),
                    Of(unit.CurrentShield)));
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
                null, null, 0);
        }
    }
}

// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;

namespace IdleWarden.Kale.Decisions
{
    public sealed class SpellSnapshot
    {
        public SpellSnapshot(
            string name,
            int manaCost,
            double cooldownRemaining,
            double castTime,
            bool canTargetDown,
            bool queued)
        {
            Name = name;
            ManaCost = manaCost;
            CooldownRemaining = cooldownRemaining;
            CastTime = castTime;
            CanTargetDown = canTargetDown;
            Queued = queued;
        }

        public string Name { get; }

        /// Read afresh every tick: a talent can drop this to zero mid-fight, so a
        /// cost cached at startup describes a spell the player no longer has.
        public int ManaCost { get; }

        public double CooldownRemaining { get; }

        public double CastTime { get; }

        public bool CanTargetDown { get; }

        public bool Queued { get; }
    }

    public sealed class KaleSnapshot
    {
        public const int TutorialDoneAt = 900;

        public KaleSnapshot(
            string scene,
            int tutorialStep,
            Magnitude gold,
            Magnitude skillPoints,
            Magnitude ruby,
            int highestLevelClear,
            int tavernCompletion,
            IReadOnlyList<SkillCandidate> nodes,
            bool battleActive,
            int floor,
            int mana,
            int manaMax,
            IReadOnlyList<UnitSnapshot> party,
            IReadOnlyList<SpellSnapshot> spells)
        {
            Scene = scene;
            TutorialStep = tutorialStep;
            Gold = gold;
            SkillPoints = skillPoints;
            Ruby = ruby;
            HighestLevelClear = highestLevelClear;
            TavernCompletion = tavernCompletion;
            Nodes = nodes ?? new SkillCandidate[0];
            BattleActive = battleActive;
            Floor = floor;
            Mana = mana;
            ManaMax = manaMax;
            Party = party ?? new UnitSnapshot[0];
            Spells = spells ?? new SpellSnapshot[0];
        }

        public string Scene { get; }

        /// Below <see cref="TutorialDoneAt"/> the game swallows clicks on anything
        /// the tutorial is not pointing at, so an agent that buys during it sees
        /// its intents quietly do nothing.
        public int TutorialStep { get; }

        public Magnitude Gold { get; }

        public Magnitude SkillPoints { get; }

        public Magnitude Ruby { get; }

        public int HighestLevelClear { get; }

        public int TavernCompletion { get; }

        public IReadOnlyList<SkillCandidate> Nodes { get; }

        public bool BattleActive { get; }

        public int Floor { get; }

        public int Mana { get; }

        public int ManaMax { get; }

        public IReadOnlyList<UnitSnapshot> Party { get; }

        public IReadOnlyList<SpellSnapshot> Spells { get; }

        public bool TutorialDone => TutorialStep >= TutorialDoneAt;

        public SpellSnapshot Spell(string name)
        {
            foreach (var spell in Spells)
            {
                if (spell != null && spell.Name == name)
                {
                    return spell;
                }
            }
            return null;
        }
    }
}
